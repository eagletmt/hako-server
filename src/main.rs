use anyhow::Context as _;
use futures_util::StreamExt as _;
use futures_util::TryStreamExt as _;

#[derive(structopt::StructOpt)]
enum Opt {
    #[structopt(about = "Upload Hako definitions to S3")]
    UploadDefinitions(UploadDefinitionsOpt),
    #[structopt(about = "Start gRPC server")]
    ApiServer(ApiServerOpt),
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
    s3_prefix: String,
}

#[derive(structopt::StructOpt)]
struct ApiServerOpt {
    #[structopt(long, env, help = "S3 bucket")]
    s3_bucket: String,
    #[structopt(long, env, help = "S3 prefix")]
    s3_prefix: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use structopt::StructOpt as _;

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,tower_http::trace=debug");
    }
    tracing_subscriber::fmt::init();

    match Opt::from_args() {
        Opt::UploadDefinitions(opt) => upload_definitions(opt).await,
        Opt::ApiServer(opt) => api_server(opt).await,
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

    let key = format!("{}/current", opt.s3_prefix);
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
        ext == "jsonnet"
    } else {
        false
    }
}

async fn upload_file(
    s3_client: &aws_sdk_s3::Client,
    opt: &UploadDefinitionsOpt,
    path: std::path::PathBuf,
) -> anyhow::Result<()> {
    let filename = path
        .file_name()
        .with_context(|| format!("failed to find filename of {}", path.display()))?
        .to_str()
        .with_context(|| format!("failed to convert to string from {}", path.display()))?;
    let app_id = filename.strip_suffix(".jsonnet").unwrap();

    let key = format!("{}/{}/{}.json", opt.s3_prefix, opt.revision, app_id);

    let state = jrsonnet_evaluator::EvaluationState::default();
    state.with_stdlib();
    state.set_manifest_format(jrsonnet_evaluator::ManifestFormat::Json(2));
    state.set_import_resolver(Box::new(jrsonnet_evaluator::FileImportResolver::default()));
    state.add_ext_var("appId".into(), jrsonnet_evaluator::Val::Str(app_id.into()));
    let code = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    let json_str = state
        .evaluate_snippet_raw(path.clone().into(), code.into())
        .and_then(|val| state.manifest(val))
        .map_err(|e| anyhow::Error::msg(state.stringify_err(&e)))
        .with_context(|| format!("failed to evaluate {}", path.display()))?;
    let _: hako_server::definition::Definition = serde_json::from_str(&json_str)
        .with_context(|| format!("failed to deserialize {}\n{}", path.display(), json_str))?;

    tracing::info!(
        "Upload {} to s3://{}/{}",
        path.display(),
        opt.s3_bucket,
        key
    );

    let body = aws_sdk_s3::ByteStream::from(json_str.as_bytes().to_owned());
    s3_client
        .put_object()
        .bucket(&opt.s3_bucket)
        .key(&key)
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

async fn api_server(opt: ApiServerOpt) -> anyhow::Result<()> {
    let shared_config = aws_config::load_from_env().await;
    let service = hako_server::api_server::DeploymentService::new(
        hako_server::api_server::DeploymentServiceConfig {
            s3_bucket: opt.s3_bucket,
            s3_prefix: opt.s3_prefix,
        },
        &shared_config,
    );
    let server = hako_server::api_server::DeploymentServer::new(service);
    let layer = tower::ServiceBuilder::new().layer(tower_http::trace::TraceLayer::new_for_grpc());
    let server = tonic::transport::Server::builder()
        .layer(layer)
        .add_service(server);
    if let Some(l) = listenfd::ListenFd::from_env().take_tcp_listener(0)? {
        let listener = tokio::net::TcpListener::from_std(l)?;
        let stream = tokio_stream::wrappers::TcpListenerStream::new(listener);
        server.serve_with_incoming(stream).await?
    } else {
        server
            .serve(std::net::SocketAddr::from(([127, 0, 0, 1], 50051)))
            .await?
    }
    Ok(())
}
