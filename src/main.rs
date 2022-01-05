use anyhow::Context as _;
use futures_util::StreamExt as _;
use futures_util::TryStreamExt as _;

#[derive(structopt::StructOpt)]
enum Opt {
    #[structopt(about = "Upload Hako definitions to S3")]
    UploadDefinitions(UploadDefinitionsOpt),
}

#[derive(structopt::StructOpt)]
struct UploadDefinitionsOpt {
    #[structopt(short, long, help = "Revision of the Hako definitions to upload")]
    revision: String,
    #[structopt(
        short,
        long,
        default_value = ".",
        help = "Directory of Hako definitions to upload"
    )]
    directory: std::path::PathBuf,
    #[structopt(long, env, help = "Target S3 bucket")]
    s3_bucket: String,
    #[structopt(long, env, help = "Target S3 prefix")]
    s3_prefix: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use structopt::StructOpt as _;
    tracing_subscriber::fmt::init();

    match Opt::from_args() {
        Opt::UploadDefinitions(opt) => upload_definitions(opt).await,
    }
}

async fn upload_definitions(opt: UploadDefinitionsOpt) -> anyhow::Result<()> {
    let shared_config = aws_config::load_from_env().await;
    let s3_client = aws_sdk_s3::Client::new(&shared_config);

    let mut futures = Vec::new();
    for entry in walkdir::WalkDir::new(&opt.directory) {
        let entry = entry.context("failed to walk directories")?;
        if !entry.file_type().is_dir() && is_jsonnet(entry.path()) {
            futures.push(upload_file(&s3_client, &opt, entry.into_path()));
        }
    }
    let mut futures_unordered = futures_util::stream::iter(futures).buffer_unordered(16);

    while futures_unordered
        .try_next()
        .await
        .context("upload_file failed")?
        .is_some()
    {}

    let key = if let Some(ref prefix) = opt.s3_prefix {
        format!("{}/current", prefix)
    } else {
        "current".to_owned()
    };
    tracing::info!(
        "Upload current ({}) to s3://{}/{}",
        opt.revision,
        opt.s3_bucket,
        key
    );
    s3_client
        .put_object()
        .bucket(&opt.s3_bucket)
        .key(&key)
        .body(format!("{}\n", opt.revision).into_bytes().into())
        .send()
        .await
        .with_context(|| {
            format!(
                "failed to upload current ({}) to s3://{}/{}",
                opt.revision, opt.s3_bucket, key
            )
        })?;

    Ok(())
}

fn is_jsonnet(path: &std::path::Path) -> bool {
    if let Some(ext) = path.extension() {
        ext == "jsonnet" || ext == "libsonnet"
    } else {
        false
    }
}

async fn upload_file(
    s3_client: &aws_sdk_s3::Client,
    opt: &UploadDefinitionsOpt,
    path: std::path::PathBuf,
) -> anyhow::Result<()> {
    let relative_path = path
        .strip_prefix(&opt.directory)
        .context("failed to strip root directory prefix")?;
    let key = if let Some(ref prefix) = opt.s3_prefix {
        format!("{}/{}/{}", prefix, opt.revision, relative_path.display())
    } else {
        format!("{}/{}", opt.revision, relative_path.display())
    };

    // TODO: validate Jsonnet content

    tracing::info!(
        "Upload {} to s3://{}/{}",
        path.display(),
        opt.s3_bucket,
        key
    );

    let metadata = tokio::fs::metadata(&path)
        .await
        .with_context(|| format!("failed to read metadata of {}", path.display()))?;
    let body = aws_sdk_s3::ByteStream::from_path(&path)
        .await
        .with_context(|| format!("failed to read {} into ByteStream", path.display()))?;
    s3_client
        .put_object()
        .bucket(&opt.s3_bucket)
        .key(&key)
        .content_length(metadata.len() as i64)
        .body(body)
        .send()
        .await
        .with_context(|| {
            format!(
                "failed to upload {} to s3://{}/{}",
                path.display(),
                opt.s3_bucket,
                key
            )
        })?;

    Ok(())
}
