use anyhow::Context as _;

mod pb {
    tonic::include_proto!("hako");
}
pub use pb::deployment_server::DeploymentServer;

#[derive(Debug)]
pub struct DeploymentService {
    config: DeploymentServiceConfig,
    s3_client: aws_sdk_s3::Client,
    ecs_client: aws_sdk_ecs::Client,
}

#[derive(Debug)]
pub struct DeploymentServiceConfig {
    pub s3_bucket: String,
    pub s3_prefix: String,
}

impl DeploymentService {
    pub fn new(config: DeploymentServiceConfig, shared_config: &aws_config::Config) -> Self {
        Self {
            config,
            s3_client: aws_sdk_s3::Client::new(shared_config),
            ecs_client: aws_sdk_ecs::Client::new(shared_config),
        }
    }
}

#[tonic::async_trait]
impl pb::deployment_server::Deployment for DeploymentService {
    async fn deploy_oneshot(
        &self,
        request: tonic::Request<pb::DeployOneshotRequest>,
    ) -> Result<tonic::Response<pb::DeployOneshotResponse>, tonic::Status> {
        if request.get_ref().app_id.is_empty() {
            return Err(tonic::Status::invalid_argument("app_id must be present"));
        }
        wrap(self.deploy_oneshot_impl(request).await).await
    }
}

async fn wrap<T>(
    result: anyhow::Result<Result<T, tonic::Status>>,
) -> Result<tonic::Response<T>, tonic::Status> {
    match result {
        Ok(Ok(r)) => Ok(tonic::Response::new(r)),
        Ok(Err(s)) => Err(s),
        Err(e) => {
            tracing::error!(?e);
            Err(tonic::Status::internal(e.to_string()))
        }
    }
}

impl DeploymentService {
    async fn deploy_oneshot_impl(
        &self,
        request: tonic::Request<pb::DeployOneshotRequest>,
    ) -> anyhow::Result<Result<pb::DeployOneshotResponse, tonic::Status>> {
        let current_key = format!("{}/current", self.config.s3_prefix);
        let resp = self
            .s3_client
            .get_object()
            .bucket(&self.config.s3_bucket)
            .key(&current_key)
            .send()
            .await
            .with_context(|| {
                format!(
                    "failed to get current object s3://{}/{}",
                    self.config.s3_bucket, current_key
                )
            })?;
        let body = resp
            .body
            .collect()
            .await
            .with_context(|| {
                format!(
                    "failed to read contents of current object s3://{}/{}",
                    self.config.s3_bucket, current_key
                )
            })?
            .into_bytes();
        let revision = String::from_utf8(body.to_vec()).with_context(|| {
            format!(
                "failed to read contents of current object s3://{}/{} as UTF-8",
                self.config.s3_bucket, current_key
            )
        })?;
        let revision = revision.trim_end();

        let key = format!(
            "{}/{}/{}.json",
            self.config.s3_prefix,
            revision,
            request.get_ref().app_id
        );
        let result = self
            .s3_client
            .get_object()
            .bucket(&self.config.s3_bucket)
            .key(&key)
            .send()
            .await;
        if matches!(
            result,
            Err(aws_sdk_s3::SdkError::ServiceError {
                err: aws_sdk_s3::error::GetObjectError {
                    kind: aws_sdk_s3::error::GetObjectErrorKind::NoSuchKey(_),
                    ..
                },
                ..
            })
        ) {
            return Ok(Err(tonic::Status::not_found(format!(
                "{} is not registered",
                request.get_ref().app_id
            ))));
        }
        let resp = result.with_context(|| {
            format!(
                "failed to get definition s3://{}/{}",
                self.config.s3_bucket, key
            )
        })?;
        let body = resp
            .body
            .collect()
            .await
            .with_context(|| {
                format!(
                    "failed to read contents of definition s3://{}/{}",
                    self.config.s3_bucket, key
                )
            })?
            .into_bytes();

        let definition: crate::definition::Definition = serde_json::from_slice(&body)
            .with_context(|| {
                format!(
                    "failed to deserialize definition s3://{}/{}",
                    self.config.s3_bucket, key
                )
            })?;
        tracing::info!(?definition);

        let builder = build_oneshot_task_definition(
            self.ecs_client.register_task_definition(),
            &request.get_ref().app_id,
            &definition,
        );
        tracing::info!(?builder);
        let resp = builder.send().await.with_context(|| {
            format!(
                "failed to register task definition from s3://{}/{}",
                self.config.s3_bucket, key
            )
        })?;
        let task_definition = resp.task_definition.unwrap();
        tracing::info!(?task_definition);

        Ok(Ok(pb::DeployOneshotResponse {
            task_definition_arn: task_definition.task_definition_arn.unwrap(),
        }))
    }
}

fn build_oneshot_task_definition<C, M, R>(
    mut builder: aws_sdk_ecs::client::fluent_builders::RegisterTaskDefinition<C, M, R>,
    app_id: &str,
    definition: &crate::definition::Definition,
) -> aws_sdk_ecs::client::fluent_builders::RegisterTaskDefinition<C, M, R>
where
    C: aws_smithy_client::bounds::SmithyConnector,
    M: aws_smithy_client::bounds::SmithyMiddleware<C>,
    R: aws_smithy_client::retry::NewRequestPolicy,
{
    use std::str::FromStr as _;

    builder = builder.family(app_id);
    if let Some(ref task_role_arn) = definition.scheduler.task_role_arn {
        builder = builder.task_role_arn(task_role_arn);
    }
    if let Some(ref execution_role_arn) = definition.scheduler.execution_role_arn {
        builder = builder.execution_role_arn(execution_role_arn);
    }
    if let Some(ref network_mode) = definition.scheduler.network_mode {
        match aws_sdk_ecs::model::NetworkMode::from_str(network_mode) {
            Ok(mode) => {
                builder = builder.network_mode(mode);
            }
            Err(e) => {
                tracing::warn!(%network_mode, ?e, "invalid .scheduler.network_mode value");
            }
        }
    }
    if let Some(ref cpu) = definition.scheduler.cpu {
        builder = builder.cpu(cpu);
    }
    if let Some(ref memory) = definition.scheduler.memory {
        builder = builder.memory(memory);
    }
    for compatibility in &definition.scheduler.requires_compatibilities {
        match aws_sdk_ecs::model::Compatibility::from_str(compatibility) {
            Ok(c) => {
                builder = builder.requires_compatibilities(c);
            }
            Err(e) => {
                tracing::warn!(%compatibility, ?e, "invalid .scheduler.requires_compatibilities[] value");
            }
        }
    }

    {
        let mut b = aws_sdk_ecs::model::ContainerDefinition::builder().name("app");
        if let Some(ref tag) = definition.app.tag {
            b = b.image(format!("{}:{}", definition.app.image, tag));
        } else {
            b = b.image(format!("{}:latest", definition.app.image));
        }
        builder = builder.container_definitions(
            build_container_definition(b, &definition.app.attributes).build(),
        );
    }
    for (name, sidecar) in &definition.sidecars {
        let b = aws_sdk_ecs::model::ContainerDefinition::builder()
            .name(name)
            .image(&sidecar.image_tag);
        builder = builder
            .container_definitions(build_container_definition(b, &sidecar.attributes).build());
    }
    builder
}

fn build_container_definition(
    mut builder: aws_sdk_ecs::model::container_definition::Builder,
    container: &crate::definition::ContainerAttributes,
) -> aws_sdk_ecs::model::container_definition::Builder {
    use std::str::FromStr as _;

    if let Some(ref log_configuration) = container.log_configuration {
        let mut b = aws_sdk_ecs::model::LogConfiguration::builder();
        match aws_sdk_ecs::model::LogDriver::from_str(&log_configuration.log_driver) {
            Ok(d) => {
                b = b.log_driver(d);
            }
            Err(e) => {
                tracing::warn!(%log_configuration.log_driver, ?e, "invalid .log_configuration.driver value");
            }
        }
        for (k, v) in &log_configuration.options {
            b = b.options(k, v);
        }
        builder = builder.log_configuration(b.build());
    }
    // TODO: Support more attributes

    builder
}
