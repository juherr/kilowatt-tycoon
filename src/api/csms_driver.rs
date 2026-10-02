//! Client for the language-neutral `csms-driver` HTTP API.

use crate::components::{BelongsToSite, Charger};
use crate::resources::{ChargerIdentityRegistry, IdentityRegistryError};
use bevy::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
use bevy::tasks::Task;
#[cfg(not(target_arch = "wasm32"))]
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct CsmsDriverConfig {
    base_url: String,
    timeout: Duration,
    security_profile: u8,
    basic_auth_password: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for CsmsDriverConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CsmsDriverConfig")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("security_profile", &self.security_profile)
            .field(
                "basic_auth_password",
                &self.basic_auth_password.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl CsmsDriverConfig {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

    pub fn new(
        base_url: impl Into<String>,
        timeout: Duration,
        security_profile: u8,
        basic_auth_password: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            timeout,
            security_profile,
            basic_auth_password,
        }
    }

    /// Read native client settings. No endpoint means provisioning is disabled.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env() -> Result<Option<Self>, CsmsDriverError> {
        let Ok(base_url) = std::env::var("CSMS_DRIVER_URL") else {
            return Ok(None);
        };
        let base_url = base_url.trim();
        if base_url.is_empty() {
            return Ok(None);
        }

        let timeout =
            match std::env::var("CSMS_DRIVER_TIMEOUT_MS") {
                Ok(value) => Duration::from_millis(value.parse().map_err(|_| {
                    CsmsDriverError::new(CsmsDriverErrorKind::InvalidConfiguration)
                })?),
                Err(_) => Self::DEFAULT_TIMEOUT,
            };
        if timeout.is_zero() {
            return Err(CsmsDriverError::new(
                CsmsDriverErrorKind::InvalidConfiguration,
            ));
        }
        let security_profile = match std::env::var("CSMS_DRIVER_SECURITY_PROFILE") {
            Ok(value) => value
                .parse::<u8>()
                .map_err(|_| CsmsDriverError::new(CsmsDriverErrorKind::InvalidConfiguration))?,
            Err(_) => 0,
        };
        let basic_auth_password = std::env::var("CSMS_DRIVER_BASIC_AUTH_PASSWORD")
            .ok()
            .filter(|value| !value.is_empty());
        Ok(Some(Self::new(
            base_url,
            timeout,
            security_profile,
            basic_auth_password,
        )))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsmsDriverErrorKind {
    InvalidConfiguration,
    InvalidInput,
    DaemonUnavailable,
    TimeoutUnknown,
    UnsupportedCapability,
    NotFound,
    Conflict,
    ForbiddenOrigin,
    CsmsTransportFailure,
    CsmsRejected,
    InvalidResponse,
    HttpFailure,
    IdentityStoreUnavailable,
    IdentitySpaceExhausted,
}

impl std::fmt::Display for CsmsDriverErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidConfiguration => "invalid CSMS driver configuration",
            Self::InvalidInput => "CSMS driver rejected invalid input",
            Self::DaemonUnavailable => "CSMS driver daemon unavailable",
            Self::TimeoutUnknown => "CSMS driver request timed out; outcome may be unknown",
            Self::UnsupportedCapability => "CSMS driver does not support the requested capability",
            Self::NotFound => "charge point not found",
            Self::Conflict => "charge point provisioning conflict",
            Self::ForbiddenOrigin => "CSMS driver refused the request origin",
            Self::CsmsTransportFailure => "CSMS transport failure",
            Self::CsmsRejected => "CSMS rejected the request",
            Self::InvalidResponse => "invalid response from CSMS driver",
            Self::HttpFailure => "CSMS driver request failed",
            Self::IdentityStoreUnavailable => "charge-point identity storage unavailable",
            Self::IdentitySpaceExhausted => "charge-point identity space exhausted",
        };
        f.write_str(message)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsmsDriverError {
    kind: CsmsDriverErrorKind,
}

#[cfg(not(target_arch = "wasm32"))]
impl CsmsDriverError {
    fn new(kind: CsmsDriverErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> CsmsDriverErrorKind {
        self.kind
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Display for CsmsDriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::error::Error for CsmsDriverError {}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct ChargePointProvisioning {
    pub cp_id: String,
    pub description: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Clone)]
pub struct CsmsDriverClient {
    config: CsmsDriverConfig,
    client: reqwest::Client,
}

#[cfg(not(target_arch = "wasm32"))]
impl CsmsDriverClient {
    pub fn new(config: CsmsDriverConfig) -> Result<Self, CsmsDriverError> {
        let parsed_url = reqwest::Url::parse(&config.base_url)
            .map_err(|_| CsmsDriverError::new(CsmsDriverErrorKind::InvalidConfiguration))?;
        if config.base_url.is_empty()
            || !matches!(parsed_url.scheme(), "http" | "https")
            || !matches!(config.security_profile, 0..=3)
            || config.timeout.is_zero()
            || matches!(config.security_profile, 1 | 2)
                != config
                    .basic_auth_password
                    .as_ref()
                    .is_some_and(|password| !password.is_empty())
            || matches!(config.security_profile, 0 | 3) && config.basic_auth_password.is_some()
        {
            return Err(CsmsDriverError::new(
                CsmsDriverErrorKind::InvalidConfiguration,
            ));
        }
        let connect_timeout = config.timeout.min(Duration::from_secs(5));
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(connect_timeout)
            .build()
            .map_err(|_| CsmsDriverError::new(CsmsDriverErrorKind::InvalidConfiguration))?;
        Ok(Self { config, client })
    }

    pub async fn provision(
        &self,
        charge_point: &ChargePointProvisioning,
    ) -> Result<(), CsmsDriverError> {
        if charge_point.cp_id.is_empty()
            || !charge_point
                .cp_id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
        {
            return Err(CsmsDriverError::new(CsmsDriverErrorKind::InvalidInput));
        }

        let driver = self.get_driver().await?;
        let supported_profiles = driver
            .charge_points
            .ok_or_else(|| CsmsDriverError::new(CsmsDriverErrorKind::UnsupportedCapability))?
            .security_profiles;
        if !supported_profiles.contains(&self.config.security_profile) {
            return Err(CsmsDriverError::new(
                CsmsDriverErrorKind::UnsupportedCapability,
            ));
        }

        let desired = self.definition(charge_point);
        match self.get_charge_point(&charge_point.cp_id).await? {
            Some(existing) if self.matches(&existing, &desired) => Ok(()),
            Some(_) => {
                self.update_charge_point(&charge_point.cp_id, &desired)
                    .await
            }
            None => match self.create_charge_point(&desired).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == CsmsDriverErrorKind::Conflict => {
                    match self.get_charge_point(&charge_point.cp_id).await? {
                        Some(existing) if self.matches(&existing, &desired) => Ok(()),
                        Some(_) => {
                            self.update_charge_point(&charge_point.cp_id, &desired)
                                .await
                        }
                        None => Err(error),
                    }
                }
                Err(error) => Err(error),
            },
        }
    }

    fn definition(&self, charge_point: &ChargePointProvisioning) -> ChargePointDefinition {
        ChargePointDefinition {
            id: charge_point.cp_id.clone(),
            registration: RegistrationStatus::Accepted,
            security: ChargePointSecurity {
                profile: self.config.security_profile,
                basic_auth_password: self.config.basic_auth_password.clone(),
            },
            description: charge_point.description.clone(),
        }
    }

    fn matches(&self, existing: &ChargePointDetails, desired: &ChargePointDefinition) -> bool {
        existing.id == desired.id
            && existing.registration == desired.registration
            && existing.security.profile == desired.security.profile
            && existing.description.as_deref() == Some(desired.description.as_str())
    }

    async fn get_driver(&self) -> Result<DriverDetails, CsmsDriverError> {
        let response = self
            .client
            .get(format!("{}/v1/driver", self.config.base_url))
            .send()
            .await
            .map_err(|error| self.map_transport_error(error))?;
        let response = self.check_response(response).await?;
        response
            .json()
            .await
            .map_err(|_| CsmsDriverError::new(CsmsDriverErrorKind::InvalidResponse))
    }

    async fn get_charge_point(
        &self,
        cp_id: &str,
    ) -> Result<Option<ChargePointDetails>, CsmsDriverError> {
        let response = self
            .client
            .get(format!("{}/v1/charge-points/{cp_id}", self.config.base_url))
            .send()
            .await
            .map_err(|error| self.map_transport_error(error))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = self.check_response(response).await?;
        let details: ChargePointDetails = response
            .json()
            .await
            .map_err(|_| CsmsDriverError::new(CsmsDriverErrorKind::InvalidResponse))?;
        if details.id != cp_id {
            return Err(CsmsDriverError::new(CsmsDriverErrorKind::InvalidResponse));
        }
        Ok(Some(details))
    }

    async fn create_charge_point(
        &self,
        definition: &ChargePointDefinition,
    ) -> Result<(), CsmsDriverError> {
        let response = self
            .client
            .post(format!("{}/v1/charge-points", self.config.base_url))
            .json(definition)
            .send()
            .await
            .map_err(|error| self.map_transport_error(error))?;
        if response.status() == reqwest::StatusCode::CONFLICT {
            return Err(self.response_error(response).await);
        }
        let response = self.check_response(response).await?;
        let created: ChargePointCreated = response
            .json()
            .await
            .map_err(|_| CsmsDriverError::new(CsmsDriverErrorKind::InvalidResponse))?;
        if created.id != definition.id {
            return Err(CsmsDriverError::new(CsmsDriverErrorKind::InvalidResponse));
        }
        Ok(())
    }

    async fn update_charge_point(
        &self,
        cp_id: &str,
        definition: &ChargePointDefinition,
    ) -> Result<(), CsmsDriverError> {
        let update = ChargePointUpdate {
            registration: definition.registration,
            security: definition.security.clone(),
            description: definition.description.clone(),
        };
        let response = self
            .client
            .patch(format!("{}/v1/charge-points/{cp_id}", self.config.base_url))
            .json(&update)
            .send()
            .await
            .map_err(|error| self.map_transport_error(error))?;
        self.check_response(response).await.map(|_| ())
    }

    async fn check_response(
        &self,
        response: reqwest::Response,
    ) -> Result<reqwest::Response, CsmsDriverError> {
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(self.response_error(response).await)
        }
    }

    async fn response_error(&self, response: reqwest::Response) -> CsmsDriverError {
        let status = response.status();
        let kind = response
            .json::<ErrorEnvelope>()
            .await
            .ok()
            .map(|envelope| kind_for_code(&envelope.error.code))
            .unwrap_or_else(|| kind_for_status(status));
        CsmsDriverError::new(kind)
    }

    fn map_transport_error(&self, error: reqwest::Error) -> CsmsDriverError {
        if error.is_timeout() {
            CsmsDriverError::new(CsmsDriverErrorKind::TimeoutUnknown)
        } else if error.is_connect() {
            CsmsDriverError::new(CsmsDriverErrorKind::DaemonUnavailable)
        } else {
            CsmsDriverError::new(CsmsDriverErrorKind::HttpFailure)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriverDetails {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    display_name: String,
    charge_points: Option<ChargePointCapabilities>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChargePointCapabilities {
    security_profiles: Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum RegistrationStatus {
    Accepted,
    Pending,
    Rejected,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChargePointSecurity {
    profile: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    basic_auth_password: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for ChargePointSecurity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChargePointSecurity")
            .field("profile", &self.profile)
            .field(
                "basic_auth_password",
                &self.basic_auth_password.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Serialize)]
struct ChargePointDefinition {
    id: String,
    registration: RegistrationStatus,
    security: ChargePointSecurity,
    description: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Serialize)]
struct ChargePointUpdate {
    registration: RegistrationStatus,
    security: ChargePointSecurity,
    description: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct ChargePointDetails {
    id: String,
    registration: RegistrationStatus,
    security: ChargePointSecurityDetails,
    description: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct ChargePointSecurityDetails {
    profile: u8,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct ChargePointCreated {
    id: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct ErrorBody {
    code: String,
}

#[cfg(not(target_arch = "wasm32"))]
fn kind_for_code(code: &str) -> CsmsDriverErrorKind {
    match code {
        "invalid_input" => CsmsDriverErrorKind::InvalidInput,
        "not_found" => CsmsDriverErrorKind::NotFound,
        "forbidden_origin" => CsmsDriverErrorKind::ForbiddenOrigin,
        "conflict" => CsmsDriverErrorKind::Conflict,
        "unsupported_capability" => CsmsDriverErrorKind::UnsupportedCapability,
        "transport_failure" => CsmsDriverErrorKind::CsmsTransportFailure,
        "csms_rejected" => CsmsDriverErrorKind::CsmsRejected,
        "timeout" => CsmsDriverErrorKind::TimeoutUnknown,
        _ => CsmsDriverErrorKind::HttpFailure,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn kind_for_status(status: reqwest::StatusCode) -> CsmsDriverErrorKind {
    match status.as_u16() {
        400 => CsmsDriverErrorKind::InvalidInput,
        403 => CsmsDriverErrorKind::ForbiddenOrigin,
        404 => CsmsDriverErrorKind::NotFound,
        409 => CsmsDriverErrorKind::Conflict,
        501 => CsmsDriverErrorKind::UnsupportedCapability,
        502 => CsmsDriverErrorKind::CsmsRejected,
        504 => CsmsDriverErrorKind::TimeoutUnknown,
        _ => CsmsDriverErrorKind::HttpFailure,
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
pub struct CsmsConfigurationError(pub CsmsDriverErrorKind);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsmsProvisioningState {
    NotConfigured,
    Provisioning,
    Provisioned,
    Failed(CsmsDriverErrorKind),
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Default)]
pub struct ProvisioningTaskRegistry(Vec<(Entity, Task<Result<(), CsmsDriverError>>)>);

fn stable_charger_key(charger: &Charger, site: Option<&BelongsToSite>) -> Option<String> {
    charger.grid_position.is_none().then(|| {
        format!(
            "authored:{}:{}",
            site.map(|tag| tag.site_id.0).unwrap_or_default(),
            charger.id
        )
    })
}

fn assign_identity(
    charger: &mut Charger,
    site: Option<&BelongsToSite>,
    identities: &mut ChargerIdentityRegistry,
) -> Result<(), CsmsDriverErrorKind> {
    identities
        .assign(stable_charger_key(charger, site).as_deref())
        .map(|cp_id| charger.cp_id = cp_id)
        .map_err(|error| match error {
            IdentityRegistryError::StorageUnavailable(_)
            | IdentityRegistryError::CorruptRegistry => {
                CsmsDriverErrorKind::IdentityStoreUnavailable
            }
            IdentityRegistryError::Exhausted => CsmsDriverErrorKind::IdentitySpaceExhausted,
        })
}

/// Assign stable identities once and start provisioning for newly-created chargers.
#[cfg(not(target_arch = "wasm32"))]
pub fn assign_charger_identities(
    mut commands: Commands,
    mut identities: ResMut<ChargerIdentityRegistry>,
    client: Option<Res<CsmsDriverClient>>,
    config_error: Option<Res<CsmsConfigurationError>>,
    mut tasks: ResMut<ProvisioningTaskRegistry>,
    mut chargers: Query<(Entity, &mut Charger, Option<&BelongsToSite>), Added<Charger>>,
) {
    for (entity, mut charger, site) in &mut chargers {
        if let Err(kind) = assign_identity(&mut charger, site, &mut identities) {
            warn!("Charger identity assignment failed: {kind}");
            commands
                .entity(entity)
                .insert(CsmsProvisioningState::Failed(kind));
            continue;
        }

        if let Some(config_error) = &config_error {
            commands
                .entity(entity)
                .insert(CsmsProvisioningState::Failed(config_error.0));
            continue;
        }
        let Some(client) = &client else {
            commands
                .entity(entity)
                .insert(CsmsProvisioningState::NotConfigured);
            continue;
        };

        let description = format!(
            "{} — {:?} {:.0} kW",
            if charger.name.is_empty() {
                charger.id.as_str()
            } else {
                charger.name.as_str()
            },
            charger.charger_type,
            charger.rated_power_kw
        );
        let provisioning = ChargePointProvisioning {
            cp_id: charger.cp_id.clone(),
            description,
        };
        let client = (*client).clone();
        let task =
            crate::ui::spawn_network_task(async move { client.provision(&provisioning).await });
        tasks.0.push((entity, task));
        commands
            .entity(entity)
            .insert(CsmsProvisioningState::Provisioning);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn assigns_a_durable_cp_id_and_keeps_local_play_available_without_daemon() {
        let mut app = App::new();
        app.insert_resource(ChargerIdentityRegistry::in_memory())
            .init_resource::<ProvisioningTaskRegistry>()
            .add_systems(Update, assign_charger_identities);
        let entity = app
            .world_mut()
            .spawn(Charger {
                id: "chg_01".to_string(),
                name: "Display label".to_string(),
                ..default()
            })
            .id();

        app.update();

        let charger = app.world().get::<Charger>(entity).unwrap();
        assert_eq!(charger.cp_id, "KT-00000001");
        assert_eq!(
            app.world().get::<CsmsProvisioningState>(entity),
            Some(&CsmsProvisioningState::NotConfigured)
        );
    }
}

/// Poll provision requests without blocking the game update loop.
#[cfg(not(target_arch = "wasm32"))]
pub fn poll_provisioning_tasks(
    mut commands: Commands,
    mut tasks: ResMut<ProvisioningTaskRegistry>,
) {
    let mut index = 0;
    while index < tasks.0.len() {
        let (entity, task) = &mut tasks.0[index];
        if let Some(result) = poll_task(task) {
            match result {
                Ok(()) => {
                    commands
                        .entity(*entity)
                        .insert(CsmsProvisioningState::Provisioned);
                }
                Err(error) => {
                    warn!("CSMS provisioning failed: {}", error.kind());
                    commands
                        .entity(*entity)
                        .insert(CsmsProvisioningState::Failed(error.kind()));
                }
            }
            let (_, completed_task) = tasks.0.swap_remove(index);
            completed_task.detach();
        } else {
            index += 1;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn poll_task<T>(task: &mut Task<T>) -> Option<T> {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    let mut context = Context::from_waker(Waker::noop());
    match Pin::new(task).poll(&mut context) {
        Poll::Ready(value) => Some(value),
        Poll::Pending => None,
    }
}

#[cfg(target_arch = "wasm32")]
pub fn assign_charger_identities(
    mut commands: Commands,
    mut identities: ResMut<ChargerIdentityRegistry>,
    mut chargers: Query<(Entity, &mut Charger, Option<&BelongsToSite>), Added<Charger>>,
) {
    for (entity, mut charger, site) in &mut chargers {
        if let Err(kind) = assign_identity(&mut charger, site, &mut identities) {
            warn!("Charger identity assignment failed: {kind}");
            commands
                .entity(entity)
                .insert(CsmsProvisioningState::Failed(kind));
            continue;
        }
        commands
            .entity(entity)
            .insert(CsmsProvisioningState::NotConfigured);
    }
}
