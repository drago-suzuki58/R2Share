use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use arboard::Clipboard;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{
    Client,
    config::{Credentials, Region},
    primitives::ByteStream,
};
use aws_smithy_types::body::SdkBody;
use chrono::Local;
use http_body_util::BodyExt;
use indicatif::{ProgressBar, ProgressStyle};
use mime_guess::mime;
use serde::Deserialize;
use ulid::Ulid;

const CONFIG_FILE_NAME: &str = "r2share.toml";
const APPDATA_CONFIG_PATH: &[&str] = &["R2Share", "config.toml"];
const CACHE_CONTROL: &str = "public, max-age=31536000";

#[derive(Debug, Deserialize)]
struct Config {
    bucket: String,
    endpoint: String,
    access_key_id: String,
    secret_access_key: String,
    public_base_url: String,
    #[serde(default = "default_prefix")]
    default_prefix: String,
}

fn default_prefix() -> String {
    "uploads".to_owned()
}

#[tokio::main]
async fn main() {
    let exit_code = match run().await {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("Upload failed.");
            eprintln!("Reason: {err:#}");
            1
        }
    };

    wait_for_exit();
    std::process::exit(exit_code);
}

async fn run() -> Result<()> {
    let file_args = collect_file_args()?;
    let config_path = find_config_path()?;
    let config = load_config(&config_path)?;
    let client = build_s3_client(&config).await;

    let mut uploaded_urls = Vec::new();
    let mut failed_files = Vec::new();
    let total_files = file_args.len();

    for (index, file_arg) in file_args.iter().enumerate() {
        match upload_file(&client, &config, file_arg, index + 1, total_files).await {
            Ok(url) => {
                println!("{url}");
                uploaded_urls.push(url);
            }
            Err(err) => failed_files.push((file_arg.display().to_string(), err)),
        }
    }

    if !uploaded_urls.is_empty() {
        copy_urls_to_clipboard(&uploaded_urls);
    }

    if failed_files.is_empty() {
        return Ok(());
    }

    eprintln!("Failed files:");
    for (path, err) in &failed_files {
        eprintln!("- {path}: {err:#}");
    }

    bail!("{} file(s) failed to upload.", failed_files.len())
}

fn find_config_path() -> Result<PathBuf> {
    let exe_path = env::current_exe().context("Failed to locate executable path")?;
    let exe_dir = exe_path
        .parent()
        .context("Executable directory could not be determined")?;
    let local_config = exe_dir.join(CONFIG_FILE_NAME);
    if local_config.is_file() {
        return Ok(local_config);
    }

    if let Some(appdata) = env::var_os("APPDATA") {
        let appdata_config = APPDATA_CONFIG_PATH
            .iter()
            .fold(PathBuf::from(appdata), |path, segment| path.join(segment));
        if appdata_config.is_file() {
            return Ok(appdata_config);
        }
    }

    bail!(
        "Configuration file was not found. Checked '{}' and '%APPDATA%\\R2Share\\config.toml'.",
        local_config.display()
    )
}

fn load_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file '{}'.", path.display()))?;
    let config: Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file '{}'.", path.display()))?;
    Ok(config)
}

fn collect_file_args() -> Result<Vec<PathBuf>> {
    let file_args = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if file_args.is_empty() {
        bail!("No file path was provided.")
    }

    Ok(file_args)
}

async fn build_s3_client(config: &Config) -> Client {
    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new("auto"))
        .load()
        .await;
    let credentials = Credentials::new(
        config.access_key_id.clone(),
        config.secret_access_key.clone(),
        None,
        None,
        "r2share-config",
    );
    let sdk_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .credentials_provider(credentials)
        .region(Region::new("auto"))
        .endpoint_url(config.endpoint.trim_end_matches('/'))
        .force_path_style(true)
        .build();

    Client::from_conf(sdk_config)
}

async fn upload_file(
    client: &Client,
    config: &Config,
    path: &Path,
    file_index: usize,
    total_files: usize,
) -> Result<String> {
    validate_input_file(path)?;

    let file_size = fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for '{}'.", path.display()))?
        .len();
    let content_type = guess_content_type(path);
    let content_disposition = select_content_disposition(&content_type);
    let object_key = build_object_key(config, path);
    let progress_bar = create_upload_progress_bar(path, file_index, total_files, file_size)?;
    let body = build_upload_body(path, progress_bar.clone()).await?;

    let upload_result = client
        .put_object()
        .bucket(&config.bucket)
        .key(&object_key)
        .body(body)
        .content_type(content_type.clone())
        .content_disposition(content_disposition)
        .cache_control(CACHE_CONTROL)
        .send()
        .await
        .with_context(|| format!("Failed to upload '{}' to R2.", path.display()));

    match upload_result {
        Ok(_) => {
            progress_bar.set_position(file_size);
            progress_bar.finish_with_message(format!(
                "[{file_index}/{total_files}] Uploaded {}",
                path_display_name(path)
            ));
        }
        Err(err) => {
            progress_bar.abandon_with_message(format!(
                "[{file_index}/{total_files}] Failed {}",
                path_display_name(path)
            ));
            return Err(err);
        }
    }

    Ok(build_public_url(&config.public_base_url, &object_key))
}

fn validate_input_file(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("File does not exist.")
    }

    if !path.is_file() {
        bail!("Directories are not supported.")
    }

    Ok(())
}

fn guess_content_type(path: &Path) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_owned()
}

fn select_content_disposition(content_type: &str) -> &'static str {
    let mime = content_type.parse::<mime::Mime>().ok();
    match mime {
        Some(mime)
            if mime.type_() == mime::IMAGE
                || mime.type_() == mime::VIDEO
                || mime.type_() == mime::AUDIO
                || mime.type_() == mime::TEXT
                || content_type == "application/json"
                || content_type == "application/pdf"
                || content_type == "application/xml" =>
        {
            "inline"
        }
        _ => "attachment",
    }
}

fn build_object_key(config: &Config, path: &Path) -> String {
    let prefix = config.default_prefix.trim_matches('/');
    let date_path = Local::now().format("%Y/%m/%d");
    let ulid = Ulid::new().to_string();

    let file_name = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{ulid}.{ext}"),
        _ => ulid,
    };

    format!("{prefix}/{date_path}/{file_name}")
}

fn build_public_url(public_base_url: &str, object_key: &str) -> String {
    format!(
        "{}/{}",
        public_base_url.trim_end_matches('/'),
        object_key.trim_start_matches('/')
    )
}

fn create_upload_progress_bar(
    path: &Path,
    file_index: usize,
    total_files: usize,
    file_size: u64,
) -> Result<ProgressBar> {
    let progress_bar = ProgressBar::new(file_size);
    progress_bar.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {bytes}/{total_bytes} {binary_bytes_per_sec} ETA {eta} {msg}",
        )?
        .progress_chars("##-"),
    );
    progress_bar.enable_steady_tick(Duration::from_millis(120));
    progress_bar.set_message(format!(
        "[{file_index}/{total_files}] {}",
        path_display_name(path)
    ));

    Ok(progress_bar)
}

fn path_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

async fn build_upload_body(path: &Path, progress_bar: ProgressBar) -> Result<ByteStream> {
    let byte_stream = ByteStream::from_path(path.to_path_buf())
        .await
        .with_context(|| format!("Failed to read file '{}'.", path.display()))?;

    Ok(byte_stream.map(move |body| add_progress_to_sdk_body(body, progress_bar.clone())))
}

fn add_progress_to_sdk_body(body: SdkBody, progress_bar: ProgressBar) -> SdkBody {
    body.map_preserve_contents(move |inner| {
        let progress_bar = progress_bar.clone();
        let mapped = inner.map_frame(move |frame| {
            if let Some(data) = frame.data_ref() {
                let next_position = progress_bar.position().saturating_add(data.len() as u64);
                progress_bar
                    .set_position(next_position.min(progress_bar.length().unwrap_or(u64::MAX)));
            }
            frame
        });

        SdkBody::from_body_1_x(mapped)
    })
}

fn copy_urls_to_clipboard(urls: &[String]) {
    let text = urls.join("\n");
    match Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text)) {
        Ok(()) => eprintln!("Copied {} URL(s) to the clipboard.", urls.len()),
        Err(err) => eprintln!("Clipboard copy failed: {err}"),
    }
}

fn wait_for_exit() {
    eprint!("Press Enter to exit...");
    let _ = io::stderr().flush();

    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            bucket: "bucket".to_owned(),
            endpoint: "https://example.r2.cloudflarestorage.com".to_owned(),
            access_key_id: "key".to_owned(),
            secret_access_key: "secret".to_owned(),
            public_base_url: "https://files.example.com".to_owned(),
            default_prefix: "uploads".to_owned(),
        }
    }

    #[test]
    fn inline_types_use_inline_disposition() {
        assert_eq!(select_content_disposition("image/png"), "inline");
        assert_eq!(select_content_disposition("video/mp4"), "inline");
        assert_eq!(select_content_disposition("text/plain"), "inline");
        assert_eq!(select_content_disposition("application/pdf"), "inline");
    }

    #[test]
    fn binary_types_use_attachment_disposition() {
        assert_eq!(select_content_disposition("application/zip"), "attachment");
        assert_eq!(
            select_content_disposition("application/octet-stream"),
            "attachment"
        );
    }

    #[test]
    fn public_url_joins_without_duplicate_slashes() {
        let url = build_public_url("https://files.example.com/", "/uploads/a.txt");
        assert_eq!(url, "https://files.example.com/uploads/a.txt");
    }

    #[test]
    fn object_key_keeps_extension() {
        let key = build_object_key(&test_config(), Path::new("movie.mp4"));
        assert!(key.starts_with("uploads/"));
        assert!(key.ends_with(".mp4"));
    }
}
