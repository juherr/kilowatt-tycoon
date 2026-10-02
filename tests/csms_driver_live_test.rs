#![cfg(not(target_arch = "wasm32"))]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kilowatt_tycoon::api::csms_driver::{
    ChargePointProvisioning, CsmsDriverClient, CsmsDriverConfig,
};

/// Run with `CSMS_DRIVER_TEST_URL=... cargo test --test csms_driver_live_test -- --ignored`.
/// The configured daemon must point at an isolated SteVe with unknown-station
/// auto-registration disabled. This client performs no OCPP connection.
#[tokio::test]
#[ignore = "requires a live csms-driver daemon and isolated SteVe instance"]
async fn provisions_a_charge_point_without_opening_an_ocpp_connection() {
    let base_url = std::env::var("CSMS_DRIVER_TEST_URL")
        .expect("CSMS_DRIVER_TEST_URL must point to a running csms-driver daemon");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after the Unix epoch")
        .as_millis();
    let cp_id = format!("KT-LIVE-{nonce}");
    let client = CsmsDriverClient::new(CsmsDriverConfig::new(
        base_url.trim_end_matches('/'),
        Duration::from_secs(60),
        0,
        None,
    ))
    .expect("live client configuration should be valid");

    client
        .provision(&ChargePointProvisioning {
            cp_id,
            description: "Kilowatt Tycoon live provisioning test".to_string(),
        })
        .await
        .expect("daemon should provision the charge point in SteVe");
}
