#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Definition {
    pub app: App,
    #[serde(default)]
    pub sidecars: std::collections::HashMap<String, Sidecar>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct App {
    pub image: String,
    pub tag: Option<String>,
    #[serde(flatten)]
    pub attributes: ContainerAttributes,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Sidecar {
    pub image_tag: String,
    #[serde(flatten)]
    pub attributes: ContainerAttributes,
}

// XXX: For setting Serde default value: https://github.com/serde-rs/serde/issues/368
fn default_as_true() -> bool {
    true
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ContainerAttributes {
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub secrets: Vec<Secret>,
    #[serde(default)]
    pub port_mappings: Vec<PortMapping>,
    #[serde(default)]
    pub mount_points: Vec<MountPoint>,
    #[serde(default)]
    pub volumes_from: Vec<VolumeFrom>,
    pub log_configuration: Option<LogConfiguration>,
    pub health_check: Option<HealthCheck>,
    #[serde(default)]
    pub ulimits: Vec<Ulimit>,
    #[serde(default)]
    pub extra_hosts: Vec<ExtraHost>,
    pub linux_parameters: Option<LinuxParameters>,
    #[serde(default)]
    pub depends_on: Vec<DependsOn>,
    pub repository_credentials: Option<RepositoryCredentials>,
    #[serde(default)]
    pub docker_labels: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub cpu: i32,
    pub memory: Option<i32>,
    pub memory_reservation: Option<i32>,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default = "default_as_true")]
    pub essential: bool,
    pub entry_point: Option<String>,
    pub command: Option<Vec<String>>,
    pub user: Option<String>,
    #[serde(default)]
    pub privileged: bool,
    #[serde(default)]
    pub readonly_root_filesystem: bool,
    #[serde(default)]
    pub docker_security_options: Vec<String>,
    #[serde(default)]
    pub system_controls: Vec<SystemControl>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Secret {
    pub name: String,
    pub value_from: String,
}

// XXX: For setting Serde default value: https://github.com/serde-rs/serde/issues/368
fn default_as_tcp() -> String {
    "tcp".to_owned()
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PortMapping {
    #[serde(default)]
    pub container_port: i32,
    #[serde(default)]
    pub host_port: i32,
    #[serde(default = "default_as_tcp")]
    pub protocol: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MountPoint {
    pub source_volume: String,
    pub container_path: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct VolumeFrom {
    pub source_container: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LogConfiguration {
    pub log_driver: String,
    #[serde(default)]
    pub options: std::collections::HashMap<String, String>,
}

fn default_interval() -> i32 {
    30
}
fn default_retries() -> i32 {
    3
}
fn default_timeout() -> i32 {
    5
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HealthCheck {
    pub command: Vec<String>,
    #[serde(default = "default_interval")]
    pub interval: i32,
    #[serde(default = "default_retries")]
    pub retries: i32,
    #[serde(default = "default_timeout")]
    pub timeout: i32,
    pub start_period: Option<i32>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Ulimit {
    pub name: String,
    pub soft_limit: i32,
    pub hard_limit: i32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExtraHost {
    pub hostname: String,
    pub ip_address: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LinuxParameters {
    pub capabilities: Option<Vec<Capability>>,
    pub devices: Option<Vec<Device>>,
    pub init_process_enabled: Option<bool>,
    pub shared_memory_size: Option<i32>,
    pub tmpfs: Option<Vec<Tmpfs>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Capability {
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub drop: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Device {
    pub host_path: String,
    pub container_path: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Tmpfs {
    pub container_path: String,
    #[serde(default)]
    pub mount_options: Vec<String>,
    pub size: i32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DependsOn {
    pub container_name: String,
    pub condition: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RepositoryCredentials {
    pub credentials_parameter: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SystemControl {
    pub namespace: String,
    pub value: String,
}
