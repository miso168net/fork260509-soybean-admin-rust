//! F4 SC-007 — business code group coverage (validation / permission / business / server).

use server_core::web::{code, res::Res};

#[test]
fn group_4xxx_validation_code_round_trip() {
    let r = Res::<()>::new_error(code::CODE_VALIDATION_REQUIRED_FIELD, "field 'email' is required");
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["code"], 4001);
    assert!((4000..5000).contains(&v["code"].as_u64().unwrap()));
}

#[test]
fn group_5xxx_permission_code_round_trip() {
    let r = Res::<()>::new_error(code::CODE_PERMISSION_CASBIN_DENY, "casbin policy denied");
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["code"], 5001);
    assert!((5000..6000).contains(&v["code"].as_u64().unwrap()));
}

#[test]
fn group_6xxx_business_code_round_trip() {
    let r = Res::<()>::new_error(code::CODE_BUSINESS_ENTITY_NOT_FOUND, "user not found");
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["code"], 6001);
    assert!((6000..7000).contains(&v["code"].as_u64().unwrap()));
}

#[test]
fn group_9xxx_server_code_round_trip() {
    let r = Res::<()>::new_error(code::CODE_SERVER_DB_ERROR, "database connection failed");
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["code"], 9001);
    assert!((9000..10000).contains(&v["code"].as_u64().unwrap()));
}
