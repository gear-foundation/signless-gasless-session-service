use sails_rs::{calls::*, gtest::calls::*, prelude::*};

use rand_core::OsRng;
use schnorrkel::Keypair;
use sessions_client::{traits::*, ActionsForSession, Config, SignatureData};

const ACTOR_ID: u64 = 42;

#[tokio::test]
async fn create_session_works() {
    let remoting = GTestRemoting::new(ACTOR_ID.into());
    remoting.system().init_logger();

    // Submit program code into the system
    let program_code_id = remoting.system().submit_code(sessions::WASM_BINARY);

    let program_factory = sessions_client::SessionsFactory::new(remoting.clone());

    let config = Config {
        gas_to_delete_session: 10_000_000_000,
        minimum_session_duration_ms: 180_000,
        s_per_block: 3,
    };

    let program_id = program_factory
        .new(config)
        .send_recv(program_code_id, b"salt")
        .await
        .unwrap();

    let mut service_client = sessions_client::Session::new(remoting.clone());

    let key = 10;

    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client
        .create_session(signature_data, None)
        .send_recv(program_id)
        .await;

    assert!(result.is_ok());

    // check session in state
    let result = service_client
        .session_for_the_account(ACTOR_ID.into())
        .recv(program_id)
        .await
        .unwrap();

    assert!(result.is_some());

    // create session with signature
    let pair: Keypair = Keypair::generate_with(OsRng);
    let data_to_sign = SignatureData {
        key: ACTOR_ID.into(),
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };
    let complete_message = [
        b"<Bytes>".to_vec(),
        data_to_sign.encode(),
        b"</Bytes>".to_vec(),
    ]
    .concat();

    let raw_signature = pair.sign_simple(b"substrate", &complete_message).to_bytes();

    let key = ActorId::from(pair.public.to_bytes());

    let signature_data = SignatureData {
        key,
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client
        .create_session(signature_data, Some(raw_signature.to_vec()))
        .send_recv(program_id)
        .await;

    assert!(result.is_ok());

    // check session in state
    let result = service_client
        .session_for_the_account(key)
        .recv(program_id)
        .await
        .unwrap();

    assert!(result.is_some());
}

#[tokio::test]
async fn create_session_failures() {
    let remoting = GTestRemoting::new(ACTOR_ID.into());
    remoting.system().init_logger();

    // Submit program code into the system
    let program_code_id = remoting.system().submit_code(sessions::WASM_BINARY);

    let program_factory = sessions_client::SessionsFactory::new(remoting.clone());

    let config = Config {
        gas_to_delete_session: 10_000_000_000,
        minimum_session_duration_ms: 180_000,
        s_per_block: 3,
    };

    let program_id = program_factory
        .new(config)
        .send_recv(program_code_id, b"salt")
        .await
        .unwrap();

    let mut service_client = sessions_client::Session::new(remoting.clone());

    // duration is less than minimum session duration
    let key = 10;

    let signature_data = SignatureData {
        key: key.into(),
        duration: 179_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client
        .create_session(signature_data, None)
        .send_recv(program_id)
        .await;

    assert!(result.is_err());

    // duration id too long (more than 400 years)
    let signature_data = SignatureData {
        key: key.into(),
        duration: 12884901888000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client
        .create_session(signature_data, None)
        .send_recv(program_id)
        .await;

    assert!(result.is_err());

    // there are no allowed actions
    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![],
    };

    let result = service_client
        .create_session(signature_data, None)
        .send_recv(program_id)
        .await;

    assert!(result.is_err());

    // the session already exists
    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client
        .create_session(signature_data, None)
        .send_recv(program_id)
        .await;

    assert!(result.is_ok());

    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client
        .create_session(signature_data, None)
        .send_recv(program_id)
        .await;

    assert!(result.is_err())
}

#[tokio::test]
async fn delete_session_from_account_works() {
    let remoting = GTestRemoting::new(ACTOR_ID.into());
    remoting.system().init_logger();

    // Submit program code into the system
    let program_code_id = remoting.system().submit_code(sessions::WASM_BINARY);

    let program_factory = sessions_client::SessionsFactory::new(remoting.clone());

    let config = Config {
        gas_to_delete_session: 10_000_000_000,
        minimum_session_duration_ms: 180_000,
        s_per_block: 3,
    };

    let program_id = program_factory
        .new(config)
        .send_recv(program_code_id, b"salt")
        .await
        .unwrap();

    let mut service_client = sessions_client::Session::new(remoting.clone());

    // duration is less than minimum session duration
    let key = 10;

    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client
        .create_session(signature_data, None)
        .send_recv(program_id)
        .await;

    assert!(result.is_ok());

    let result = service_client
        .delete_session_from_account()
        .send_recv(program_id)
        .await;

    assert!(result.is_ok());

    // check state
    let result = service_client
        .session_for_the_account(ACTOR_ID.into())
        .recv(program_id)
        .await
        .unwrap();

    assert!(result.is_none());
}
