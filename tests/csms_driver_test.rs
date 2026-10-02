#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use kilowatt_tycoon::api::csms_driver::{
    ChargePointProvisioning, CsmsDriverClient, CsmsDriverConfig, CsmsDriverErrorKind,
};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config(url: String) -> CsmsDriverConfig {
    CsmsDriverConfig::new(url, Duration::from_secs(2), 0, None)
}

fn provisioning() -> ChargePointProvisioning {
    ChargePointProvisioning {
        cp_id: "KT-00000001".to_string(),
        description: "Charger 1 — DC Fast 50 kW".to_string(),
    }
}

async fn mount_driver(server: &MockServer, profiles: &[u8]) {
    Mock::given(method("GET"))
        .and(path("/v1/driver"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "steve",
            "displayName": "SteVe",
            "chargePoints": { "securityProfiles": profiles }
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn provisions_a_missing_charge_point() {
    let server = MockServer::start().await;
    mount_driver(&server, &[0]).await;
    Mock::given(method("GET"))
        .and(path("/v1/charge-points/KT-00000001"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": { "code": "not_found", "message": "missing" }
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/charge-points"))
        .and(body_json(serde_json::json!({
            "id": "KT-00000001",
            "registration": "Accepted",
            "security": { "profile": 0 },
            "description": "Charger 1 — DC Fast 50 kW"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "KT-00000001"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = CsmsDriverClient::new(config(server.uri())).unwrap();
    client.provision(&provisioning()).await.unwrap();
}

#[tokio::test]
async fn accepts_an_existing_matching_charge_point_without_writing() {
    let server = MockServer::start().await;
    mount_driver(&server, &[0]).await;
    Mock::given(method("GET"))
        .and(path("/v1/charge-points/KT-00000001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "KT-00000001",
            "registration": "Accepted",
            "security": { "profile": 0 },
            "description": "Charger 1 — DC Fast 50 kW"
        })))
        .mount(&server)
        .await;

    let client = CsmsDriverClient::new(config(server.uri())).unwrap();
    client.provision(&provisioning()).await.unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn patches_only_when_existing_generic_metadata_differs() {
    let server = MockServer::start().await;
    mount_driver(&server, &[0]).await;
    Mock::given(method("GET"))
        .and(path("/v1/charge-points/KT-00000001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "KT-00000001",
            "registration": "Rejected",
            "security": { "profile": 0 },
            "description": "old"
        })))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/charge-points/KT-00000001"))
        .and(body_json(serde_json::json!({
            "registration": "Accepted",
            "security": { "profile": 0 },
            "description": "Charger 1 — DC Fast 50 kW"
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = CsmsDriverClient::new(config(server.uri())).unwrap();
    client.provision(&provisioning()).await.unwrap();
}

#[tokio::test]
async fn reports_unsupported_security_profiles_before_charge_point_reads() {
    let server = MockServer::start().await;
    mount_driver(&server, &[]).await;

    let client = CsmsDriverClient::new(config(server.uri())).unwrap();
    let error = client.provision(&provisioning()).await.unwrap_err();
    assert_eq!(error.kind(), CsmsDriverErrorKind::UnsupportedCapability);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn maps_validation_and_csms_errors_to_typed_failures() {
    let validation_server = MockServer::start().await;
    mount_driver(&validation_server, &[0]).await;
    Mock::given(method("GET"))
        .and(path("/v1/charge-points/KT-00000001"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&validation_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/charge-points"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": { "code": "invalid_input", "message": "invalid" }
        })))
        .mount(&validation_server)
        .await;
    let client = CsmsDriverClient::new(config(validation_server.uri())).unwrap();
    assert_eq!(
        client.provision(&provisioning()).await.unwrap_err().kind(),
        CsmsDriverErrorKind::InvalidInput
    );

    let rejected_server = MockServer::start().await;
    mount_driver(&rejected_server, &[0]).await;
    Mock::given(method("GET"))
        .and(path("/v1/charge-points/KT-00000001"))
        .respond_with(ResponseTemplate::new(502).set_body_json(serde_json::json!({
            "error": { "code": "csms_rejected", "message": "rejected" }
        })))
        .mount(&rejected_server)
        .await;
    let client = CsmsDriverClient::new(config(rejected_server.uri())).unwrap();
    assert_eq!(
        client.provision(&provisioning()).await.unwrap_err().kind(),
        CsmsDriverErrorKind::CsmsRejected
    );
}

#[tokio::test]
async fn does_not_expose_a_configured_basic_auth_password_in_errors() {
    let server = MockServer::start().await;
    mount_driver(&server, &[1]).await;
    Mock::given(method("GET"))
        .and(path("/v1/charge-points/KT-00000001"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/charge-points"))
        .respond_with(ResponseTemplate::new(502).set_body_json(serde_json::json!({
            "error": { "code": "csms_rejected", "message": "password-do-not-leak" }
        })))
        .mount(&server)
        .await;
    let config = CsmsDriverConfig::new(
        server.uri(),
        Duration::from_secs(2),
        1,
        Some("password-do-not-leak".to_string()),
    );
    let client = CsmsDriverClient::new(config).unwrap();
    let error = client.provision(&provisioning()).await.unwrap_err();
    assert!(!error.to_string().contains("password-do-not-leak"));
    assert!(!format!("{error:?}").contains("password-do-not-leak"));
}

#[tokio::test]
async fn times_out_with_an_unknown_outcome() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/driver"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_json(serde_json::json!({
                    "id": "steve",
                    "displayName": "SteVe",
                    "chargePoints": { "securityProfiles": [0] }
                })),
        )
        .mount(&server)
        .await;
    let config = CsmsDriverConfig::new(server.uri(), Duration::from_millis(10), 0, None);
    let client = CsmsDriverClient::new(config).unwrap();
    assert_eq!(
        client.provision(&provisioning()).await.unwrap_err().kind(),
        CsmsDriverErrorKind::TimeoutUnknown
    );
}

#[tokio::test]
async fn maps_an_unavailable_daemon() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let client = CsmsDriverClient::new(config(format!("http://{address}"))).unwrap();
    assert_eq!(
        client.provision(&provisioning()).await.unwrap_err().kind(),
        CsmsDriverErrorKind::DaemonUnavailable
    );
}
