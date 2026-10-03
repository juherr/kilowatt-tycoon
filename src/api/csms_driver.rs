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
            || matches!(config.security_profile, 1 | 2)
                && parsed_url.scheme() != "https"
                && !is_loopback_http(&parsed_url)
        {
            return Err(CsmsDriverError::new(
                CsmsDriverErrorKind::InvalidConfiguration,
            ));
        }
        let connect_timeout = config.timeout.min(Duration::from_secs(5));
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
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
            && desired.security.basic_auth_password.is_none()
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
fn is_loopback_http(url: &reqwest::Url) -> bool {
    if url.scheme() != "http" {
        return false;
    }
    url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    })
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
    let site_id = site.map(|tag| tag.site_id.0).unwrap_or_default();
    charger
        .grid_instance_id
        .as_ref()
        .map(|instance_id| format!("grid:{site_id}:{instance_id}"))
        .or_else(|| {
            charger
                .grid_position
                .is_none()
                .then(|| format!("authored:{site_id}:{}", charger.id))
        })
}

fn assign_identity(
    charger: &mut Charger,
    site: Option<&BelongsToSite>,
    identities: &mut ChargerIdentityRegistry,
) -> Result<(), IdentityRegistryError> {
    identities
        .assign(stable_charger_key(charger, site).as_deref())
        .map(|cp_id| charger.cp_id = cp_id)
}

fn identity_error_kind(error: &IdentityRegistryError) -> CsmsDriverErrorKind {
    match error {
        IdentityRegistryError::StorageUnavailable(_) | IdentityRegistryError::CorruptRegistry => {
            CsmsDriverErrorKind::IdentityStoreUnavailable
        }
        IdentityRegistryError::Exhausted => CsmsDriverErrorKind::IdentitySpaceExhausted,
    }
}

#[derive(Component)]
#[doc(hidden)]
pub struct IdentityAssignmentRetry {
    attempts: u8,
    retry_at: std::time::Instant,
}

const MAX_IDENTITY_RETRIES: u8 = 4;

fn next_identity_retry(attempts: u8) -> IdentityAssignmentRetry {
    let delay = std::time::Duration::from_secs(1_u64 << attempts.min(6));
    IdentityAssignmentRetry {
        attempts,
        retry_at: std::time::Instant::now() + delay,
    }
}

/// Assign stable identities once and start provisioning for newly-created chargers.
#[cfg(not(target_arch = "wasm32"))]
pub fn assign_charger_identities(
    mut commands: Commands,
    mut identities: ResMut<ChargerIdentityRegistry>,
    client: Option<Res<CsmsDriverClient>>,
    config_error: Option<Res<CsmsConfigurationError>>,
    mut tasks: ResMut<ProvisioningTaskRegistry>,
    mut chargers: Query<
        (
            Entity,
            &mut Charger,
            Option<&BelongsToSite>,
            Option<&CsmsProvisioningState>,
            Option<&mut IdentityAssignmentRetry>,
        ),
        Or<(Added<Charger>, With<IdentityAssignmentRetry>)>,
    >,
) {
    for (entity, mut charger, site, state, retry) in &mut chargers {
        if !charger.cp_id.is_empty() && state.is_some() {
            continue;
        }
        if charger.cp_id.is_empty() {
            if retry
                .as_ref()
                .is_some_and(|retry| retry.retry_at > std::time::Instant::now())
            {
                continue;
            }
            if let Err(error) = assign_identity(&mut charger, site, &mut identities) {
                let kind = identity_error_kind(&error);
                warn!("Charger identity assignment failed: {kind}");
                let mut entity_commands = commands.entity(entity);
                entity_commands.insert(CsmsProvisioningState::Failed(kind));
                if matches!(error, IdentityRegistryError::StorageUnavailable(_)) {
                    let attempts = retry.as_ref().map_or(0, |retry| retry.attempts + 1);
                    if attempts < MAX_IDENTITY_RETRIES {
                        entity_commands.insert(next_identity_retry(attempts));
                    } else {
                        entity_commands.remove::<IdentityAssignmentRetry>();
                    }
                } else {
                    entity_commands.remove::<IdentityAssignmentRetry>();
                }
                continue;
            }
            commands.entity(entity).remove::<IdentityAssignmentRetry>();
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

    #[test]
    fn retries_identity_store_failures_without_reallocating_on_configuration_errors() {
        let mut app = App::new();
        app.insert_resource(ChargerIdentityRegistry::in_memory_with_one_failed_save())
            .init_resource::<ProvisioningTaskRegistry>()
            .add_systems(Update, assign_charger_identities);
        let entity = app.world_mut().spawn(Charger::default()).id();

        app.update();
        assert!(app.world().get::<Charger>(entity).unwrap().cp_id.is_empty());
        assert_eq!(
            app.world().get::<CsmsProvisioningState>(entity),
            Some(&CsmsProvisioningState::Failed(
                CsmsDriverErrorKind::IdentityStoreUnavailable
            ))
        );

        app.world_mut()
            .get_mut::<IdentityAssignmentRetry>(entity)
            .unwrap()
            .retry_at = std::time::Instant::now() - std::time::Duration::from_millis(1);
        app.update();
        assert_eq!(
            app.world().get::<Charger>(entity).unwrap().cp_id,
            "KT-00000001"
        );
        assert_eq!(
            app.world()
                .resource::<ChargerIdentityRegistry>()
                .allocated_count(),
            1
        );

        app.insert_resource(ChargerIdentityRegistry::in_memory())
            .insert_resource(CsmsConfigurationError(
                CsmsDriverErrorKind::InvalidConfiguration,
            ));
        let config_error_entity = app.world_mut().spawn(Charger::default()).id();
        app.update();
        app.update();
        assert_eq!(
            app.world()
                .resource::<ChargerIdentityRegistry>()
                .allocated_count(),
            1
        );
        assert_eq!(
            app.world()
                .get::<CsmsProvisioningState>(config_error_entity),
            Some(&CsmsProvisioningState::Failed(
                CsmsDriverErrorKind::InvalidConfiguration
            ))
        );
    }

    #[test]
    fn persistent_identity_store_failure_is_throttled_and_retries_are_bounded() {
        use std::sync::atomic::AtomicUsize;

        let saves = std::sync::Arc::new(AtomicUsize::new(0));
        let mut app = App::new();
        app.insert_resource(
            ChargerIdentityRegistry::in_memory_with_persistent_save_failure(saves.clone()),
        )
        .init_resource::<ProvisioningTaskRegistry>()
        .add_systems(Update, assign_charger_identities);
        let entity = app.world_mut().spawn(Charger::default()).id();

        app.update();
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(saves.load(std::sync::atomic::Ordering::Relaxed), 1);

        // Advance the retry deadline directly so the test covers the bounded
        // retry policy without sleeping through its exponential backoff.
        for _ in 0..MAX_IDENTITY_RETRIES {
            {
                let mut retry = app
                    .world_mut()
                    .get_mut::<IdentityAssignmentRetry>(entity)
                    .expect("transient storage failures retain a retry marker");
                retry.retry_at = std::time::Instant::now() - std::time::Duration::from_millis(1);
            }
            app.update();
        }
        assert!(app.world().get::<IdentityAssignmentRetry>(entity).is_none());
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(
            saves.load(std::sync::atomic::Ordering::Relaxed),
            usize::from(MAX_IDENTITY_RETRIES) + 1
        );
        assert!(app.world().get::<Charger>(entity).unwrap().cp_id.is_empty());
    }

    #[test]
    fn grid_reconstruction_keeps_identity_and_replacement_gets_a_new_one() {
        let mut grid = crate::resources::SiteGrid::default();
        grid.set_tile_content(5, 5, crate::resources::TileContent::ParkingBaySouth);
        grid.set_tile_content(5, 6, crate::resources::TileContent::Lot);
        grid.place_charger(5, 5, crate::resources::ChargerPadType::L2)
            .unwrap();
        let original_instance = grid.ensure_charger_instance_id(5, 6).unwrap();

        let mut app = App::new();
        app.insert_resource(ChargerIdentityRegistry::in_memory())
            .init_resource::<ProvisioningTaskRegistry>()
            .add_systems(Update, assign_charger_identities);
        let spawn = |app: &mut App, instance_id: String| {
            app.world_mut()
                .spawn(Charger {
                    id: "chg_grid".to_string(),
                    grid_position: Some((5, 6)),
                    grid_instance_id: Some(instance_id),
                    ..default()
                })
                .id()
        };

        let first = spawn(&mut app, original_instance.clone());
        app.update();
        let first_cp_id = app.world().get::<Charger>(first).unwrap().cp_id.clone();
        app.world_mut().despawn(first);

        let reconstructed = spawn(&mut app, original_instance.clone());
        app.update();
        assert_eq!(
            app.world().get::<Charger>(reconstructed).unwrap().cp_id,
            first_cp_id
        );

        grid.sell(5, 6).unwrap();
        grid.place_charger(5, 5, crate::resources::ChargerPadType::L2)
            .unwrap();
        let replacement_instance = grid.ensure_charger_instance_id(5, 6).unwrap();
        assert_ne!(replacement_instance, original_instance);
        app.world_mut().despawn(reconstructed);

        let replacement = spawn(&mut app, replacement_instance);
        app.update();
        assert_ne!(
            app.world().get::<Charger>(replacement).unwrap().cp_id,
            first_cp_id
        );
    }

    #[test]
    fn completed_provisioning_task_ignores_a_removed_charger() {
        let pool = bevy::tasks::TaskPool::new();
        let (complete_tx, complete_rx) = std::sync::mpsc::channel();
        let task = pool.spawn(async move {
            complete_tx.send(()).unwrap();
            Ok::<(), CsmsDriverError>(())
        });
        complete_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("task should complete");

        let mut app = App::new();
        app.init_resource::<ProvisioningTaskRegistry>()
            .add_systems(Update, poll_provisioning_tasks);
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(entity);
        app.world_mut()
            .resource_mut::<ProvisioningTaskRegistry>()
            .0
            .push((entity, task));

        for _ in 0..100 {
            app.update();
            if app
                .world()
                .resource::<ProvisioningTaskRegistry>()
                .0
                .is_empty()
            {
                break;
            }
            std::thread::yield_now();
        }
        assert!(
            app.world()
                .resource::<ProvisioningTaskRegistry>()
                .0
                .is_empty()
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
                        .try_insert(CsmsProvisioningState::Provisioned);
                }
                Err(error) => {
                    warn!("CSMS provisioning failed: {}", error.kind());
                    commands
                        .entity(*entity)
                        .try_insert(CsmsProvisioningState::Failed(error.kind()));
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
    mut chargers: Query<
        (
            Entity,
            &mut Charger,
            Option<&BelongsToSite>,
            Option<&CsmsProvisioningState>,
            Option<&mut IdentityAssignmentRetry>,
        ),
        Or<(Added<Charger>, With<IdentityAssignmentRetry>)>,
    >,
) {
    for (entity, mut charger, site, state, retry) in &mut chargers {
        if !charger.cp_id.is_empty() && state.is_some() {
            continue;
        }
        if charger.cp_id.is_empty() {
            if retry
                .as_ref()
                .is_some_and(|retry| retry.retry_at > std::time::Instant::now())
            {
                continue;
            }
            if let Err(error) = assign_identity(&mut charger, site, &mut identities) {
                let kind = identity_error_kind(&error);
                warn!("Charger identity assignment failed: {kind}");
                let mut entity_commands = commands.entity(entity);
                entity_commands.insert(CsmsProvisioningState::Failed(kind));
                if matches!(error, IdentityRegistryError::StorageUnavailable(_)) {
                    let attempts = retry.as_ref().map_or(0, |retry| retry.attempts + 1);
                    if attempts < MAX_IDENTITY_RETRIES {
                        entity_commands.insert(next_identity_retry(attempts));
                    } else {
                        entity_commands.remove::<IdentityAssignmentRetry>();
                    }
                } else {
                    entity_commands.remove::<IdentityAssignmentRetry>();
                }
                continue;
            }
            commands.entity(entity).remove::<IdentityAssignmentRetry>();
        }
        commands
            .entity(entity)
            .insert(CsmsProvisioningState::NotConfigured);
    }
}
