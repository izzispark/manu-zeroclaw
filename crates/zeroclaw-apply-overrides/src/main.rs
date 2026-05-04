use anyhow::{Context, Result};
use clap::Parser;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use toml::Value;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about = "Apply TOML config overlays to ZeroClaw base config")]
struct Args {
    #[arg(long, default_value = "/zeroclaw-data/.zeroclaw/config.toml")]
    base: String,

    #[arg(long, default_value = "/zeroclaw-data/.zeroclaw/overrides")]
    overrides_dir: String,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let base_path = Path::new(&args.base);
    let overrides_dir = Path::new(&args.overrides_dir);

    if !base_path.exists() {
        anyhow::bail!("Base config does not exist: {}", args.base);
    }

    let base_content = fs::read_to_string(base_path)
        .with_context(|| format!("Failed to read base config: {}", args.base))?;
    let mut base: Value = base_content
        .parse()
        .with_context(|| format!("Failed to parse base config as TOML: {}", args.base))?;

    let base_table = base
        .as_table_mut()
        .context("Base config must be a TOML table")?;

    if !overrides_dir.exists() {
        tracing::info!(
            "Overrides directory does not exist, skipping: {}",
            args.overrides_dir
        );
        return Ok(());
    }

    let override_files: Vec<_> = WalkDir::new(overrides_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().is_some_and(|ext| ext == "toml")
                && e.path().file_name().is_some_and(|n| n != "config.toml")
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    if override_files.is_empty() {
        tracing::info!("No override files found in {}", args.overrides_dir);
        return Ok(());
    }

    let override_files: Vec<_> = override_files
        .into_iter()
        .collect::<BTreeMap<_, _>>()
        .into_keys()
        .collect::<Vec<_>>();

    tracing::info!(count = override_files.len(), "Applying overrides");

    for override_path in &override_files {
        let override_name = override_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let override_content = fs::read_to_string(override_path)
            .with_context(|| format!("Failed to read override: {}", override_path.display()))?;
        let override_value: Value = override_content.parse().with_context(|| {
            format!(
                "Failed to parse override as TOML: {}",
                override_path.display()
            )
        })?;

        let override_index = override_files
            .iter()
            .position(|p| p == override_path)
            .unwrap_or(0);

        tracing::debug!(file = override_name, "Merging override");
        deep_merge(
            base_table,
            override_value.as_table().unwrap(),
            override_index,
            &override_files,
        );
        tracing::info!(file = override_name, "Applied override");
    }

    let merged = toml::Value::Table(base_table.clone());
    let merged_str =
        toml::to_string_pretty(&merged).context("Failed to serialize merged config")?;

    let metadata =
        fs::metadata(base_path).with_context(|| format!("Failed to stat base config: {}", args.base))?;

    fs::write(base_path, merged_str)
        .with_context(|| format!("Failed to write merged config: {}", args.base))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        fs::set_permissions(base_path, std::fs::Permissions::from_mode(mode))
            .context("Failed to restore file permissions")?;
    }

    tracing::info!("Overrides applied successfully");
    Ok(())
}

fn deep_merge(
    base: &mut toml::map::Map<String, Value>,
    overlay: &toml::map::Map<String, Value>,
    current_index: usize,
    all_files: &[std::path::PathBuf],
) {
    for (key, overlay_value) in overlay {
        match (base.get(key), overlay_value) {
            (Some(base_table @ Value::Table(_)), Value::Table(overlay_table)) => {
                deep_merge(
                    base_table.as_table_mut().unwrap(),
                    overlay_table,
                    current_index,
                    all_files,
                );
            }
            (Some(base_arr @ Value::Array(_)), Value::Array(overlay_arr)) => {
                let base_list = base_arr.as_array_mut().unwrap();
                let base_len = base_list.len();
                base_list.extend(overlay_arr.iter().cloned());
                tracing::debug!(
                    key = %key,
                    base_entries = base_len,
                    added_entries = overlay_arr.len(),
                    total = base_list.len(),
                    "Array concatenated"
                );
            }
            (Some(base_value), overlay_value) => {
                if !base_value.eq(overlay_value) {
                    if let Some(later_file) = all_files
                        .iter()
                        .enumerate()
                        .find(|(i, p)| *i > current_index && p != all_files.get(current_index))
                        .and_then(|(i, p)| {
                            Some((
                                i,
                                p.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                            ))
                        })
                    {
                        tracing::debug!(
                            key = %key,
                            base_value = %base_value,
                            overlay_value = %overlay_value,
                            override_file = %all_files
                                .get(current_index)
                                .and_then(|p| p.file_name().and_then(|n| n.to_str()))
                                .unwrap_or("?"),
                            later_file = %later_file.1,
                            "Scalar/table overridden (later file wins)"
                        );
                    }
                }
                base.insert(key.clone(), overlay_value.clone());
            }
            (None, _) => {
                base.insert(key.clone(), overlay_value.clone());
                tracing::debug!(key = %key, "New key created by overlay");
            }
        }
    }
}
