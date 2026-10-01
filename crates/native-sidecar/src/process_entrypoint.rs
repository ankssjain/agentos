//! Shared entry point for binaries embedding the native sidecar process mode.
//!
//! The executable owns adoption of its inherited control descriptor. This
//! module owns the sidecar arguments and package-cache setup so the standalone
//! agentOS binary and managed-service binary cannot drift.

use std::os::fd::OwnedFd;

fn parse_runtime_config(
    mut args: impl Iterator<Item = String>,
) -> Result<agentos_runtime::RuntimeConfig, String> {
    let mut config = agentos_runtime::RuntimeConfig::default();
    while let Some(argument) = args.next() {
        let value = if argument == "--max-active-vms" {
            args.next()
                .ok_or_else(|| String::from("--max-active-vms requires a positive integer"))?
        } else if let Some(value) = argument.strip_prefix("--max-active-vms=") {
            value.to_owned()
        } else {
            return Err(format!("unknown agentOS sidecar argument: {argument}"));
        };
        let maximum = value.parse::<usize>().map_err(|_| {
            format!("--max-active-vms must be a positive integer, received {value:?}")
        })?;
        if maximum == 0 {
            return Err(String::from(
                "--max-active-vms must be greater than zero when configured",
            ));
        }
        config.max_active_vm_executors = Some(maximum);
    }
    config.validate().map_err(|error| error.to_string())?;
    Ok(config)
}

fn parse_sidecar_options(
    args: impl Iterator<Item = String>,
) -> Result<
    (
        agentos_runtime::RuntimeConfig,
        Option<agentos_client::ProcessPackageCacheOptions>,
    ),
    String,
> {
    let mut runtime_args = Vec::new();
    let mut cache = agentos_client::ProcessPackageCacheOptions::default();
    let mut cache_configured = false;
    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        let (name, inline_value) = argument
            .split_once('=')
            .map_or((argument.as_str(), None), |(name, value)| {
                (name, Some(value))
            });
        let value = match inline_value {
            Some(value) => value.to_owned(),
            None => args
                .next()
                .ok_or_else(|| format!("{name} requires a value"))?,
        };
        if name == "--max-active-vms" {
            runtime_args.extend([name.to_owned(), value]);
            continue;
        }
        cache_configured = true;
        match name {
            "--package-cache-dir" => {
                if value.is_empty() {
                    return Err("--package-cache-dir must not be empty".into());
                }
                cache.root = Some(std::path::PathBuf::from(value));
            }
            "--package-cache-min-free-bytes" => {
                cache.min_free_bytes = parse_cache_min_free_bytes(name, &value)?;
            }
            "--package-cache-max-bytes" => {
                cache.max_bytes = parse_positive_option(name, &value)?;
            }
            "--package-cache-max-entries" => {
                cache.max_entries = parse_positive_option(name, &value)?;
            }
            "--package-cache-max-concurrent-acquisitions" => {
                cache.max_concurrent_acquisitions = parse_positive_option(name, &value)?;
            }
            "--package-cache-max-pending-acquisitions" => {
                cache.max_pending_acquisitions = parse_positive_option(name, &value)?;
            }
            "--package-cache-acquisition-timeout-ms" => {
                cache.acquisition_timeout_ms = parse_positive_option(name, &value)?;
            }
            "--package-cache-max-source-entries" => {
                cache.max_source_entries = parse_positive_option(name, &value)?;
            }
            "--package-cache-source-ttl-ms" => {
                cache.source_ttl_ms = parse_positive_option(name, &value)?;
            }
            _ => return Err(format!("unknown agentOS sidecar argument: {argument}")),
        }
    }
    let runtime_config = parse_runtime_config(runtime_args.into_iter())?;
    if cache_configured {
        cache.validate().map_err(|error| error.to_string())?;
    }
    Ok((runtime_config, cache_configured.then_some(cache)))
}

fn parse_cache_min_free_bytes(name: &str, value: &str) -> Result<u64, String> {
    // Zero intentionally disables the disk reserve; unlike queue/size caps it
    // does not make acquisition unbounded (the max-byte cap still applies).
    value
        .parse()
        .map_err(|_| format!("{name} requires a non-negative integer, received {value:?}"))
}

fn parse_positive_option<T>(name: &str, value: &str) -> Result<T, String>
where
    T: std::str::FromStr + PartialEq + From<u8>,
{
    let parsed = value
        .parse::<T>()
        .map_err(|_| format!("{name} requires a positive integer, received {value:?}"))?;
    if parsed == T::from(0) {
        return Err(format!("{name} must be greater than zero"));
    }
    Ok(parsed)
}

/// Run the native sidecar protocol on an already-adopted control descriptor.
pub fn run(control_fd: OwnedFd, args: impl Iterator<Item = String>) -> Result<(), String> {
    let (runtime_config, package_cache) = parse_sidecar_options(args)?;
    if let Some(package_cache) = package_cache {
        agentos_client::configure_process_package_cache(package_cache)
            .map_err(|error| format!("configure sidecar package cache: {error}"))?;
    }
    crate::stdio::run_with_runtime_config(control_fd, runtime_config)
        .map_err(|error| format!("run agentOS native sidecar: {error:#}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_runtime_config, parse_sidecar_options};

    #[test]
    fn runtime_executor_limit_is_uncapped_by_default_and_configurable() {
        let default = parse_runtime_config(std::iter::empty()).expect("parse default config");
        assert_eq!(default.max_active_vm_executors, None);

        let configured =
            parse_runtime_config([String::from("--max-active-vms"), String::from("7")].into_iter())
                .expect("parse configured executor limit");
        assert_eq!(configured.max_active_vm_executors, Some(7));

        let error = parse_runtime_config([String::from("--max-active-vms=0")].into_iter())
            .expect_err("zero executor limit must fail");
        assert!(error.contains("greater than zero"));
    }

    #[test]
    fn cache_minimum_disk_reserve_accepts_explicit_zero() {
        let (_, cache) = super::parse_sidecar_options(
            [String::from("--package-cache-min-free-bytes=0")].into_iter(),
        )
        .unwrap();
        assert_eq!(cache.unwrap().min_free_bytes, 0);
    }

    #[test]
    fn sidecar_accepts_worker_package_cache_settings() {
        let (runtime, cache) = parse_sidecar_options(
            [
                String::from("--max-active-vms=7"),
                String::from("--package-cache-dir=/tmp/agentos-child-cache-test"),
                String::from("--package-cache-max-entries=4"),
            ]
            .into_iter(),
        )
        .expect("parse child sidecar options");
        assert_eq!(runtime.max_active_vm_executors, Some(7));
        let cache = cache.expect("package cache override");
        assert_eq!(cache.max_entries, 4);
        assert_eq!(
            cache.root,
            Some(std::path::PathBuf::from("/tmp/agentos-child-cache-test"))
        );
        assert!(
            parse_sidecar_options([String::from("--package-cache-max-entries=0")].into_iter())
                .is_err()
        );
    }
}
