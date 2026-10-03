//! API integration module for external services

pub mod csms_driver;
pub mod leaderboard;
pub mod supabase;

use bevy::prelude::*;

pub use leaderboard::*;
pub use supabase::*;

/// Plugin that sets up API integrations
pub struct ApiPlugin;

impl Plugin for ApiPlugin {
    fn build(&self, app: &mut App) {
        if let Some(config) = SupabaseConfig::from_env() {
            info!("Supabase configured: {}", config.url);
            app.insert_resource(config);
        } else {
            info!("Supabase not configured -- leaderboard disabled");
        }

        app.insert_resource(crate::resources::ChargerIdentityRegistry::load_default());
        #[cfg(not(target_arch = "wasm32"))]
        {
            match csms_driver::CsmsDriverConfig::from_env() {
                Ok(Some(config)) => match csms_driver::CsmsDriverClient::new(config) {
                    Ok(client) => {
                        info!("CSMS driver configured");
                        app.insert_resource(client);
                    }
                    Err(error) => {
                        warn!("CSMS driver configuration invalid: {error}");
                        app.insert_resource(csms_driver::CsmsConfigurationError(error.kind()));
                    }
                },
                Ok(None) => info!("CSMS driver not configured -- provisioning disabled"),
                Err(error) => {
                    warn!("CSMS driver configuration invalid: {error}");
                    app.insert_resource(csms_driver::CsmsConfigurationError(error.kind()));
                }
            }
            app.init_resource::<csms_driver::ProvisioningTaskRegistry>()
                .add_systems(
                    Update,
                    (
                        csms_driver::assign_charger_identities,
                        csms_driver::poll_provisioning_tasks,
                    ),
                );
        }
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Update, csms_driver::assign_charger_identities);
    }
}
