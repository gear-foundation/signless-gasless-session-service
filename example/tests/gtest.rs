use rand_core::OsRng;
use sails_rs::futures::StreamExt;
use sails_rs::gtest::constants::{DEFAULT_USERS_INITIAL_BALANCE, DEFAULT_USER_ALICE};
use sails_rs::{client::*, gtest::*, ActorId, CodeId, Encode};
use schnorrkel::Keypair;
use sessions_client::session::events::SessionEvents;
use sessions_client::{
    session::*, ActionsForSession, SessionConfig, SessionsClient, SessionsClientCtors, SignatureData,
};

fn create_env() -> (GtestEnv, CodeId) {
    let system = System::new();
    system.init_logger_with_default_filter("gwasm=debug,gtest=info,sails_rs=debug,redirect=debug");
    system.mint_to(DEFAULT_USER_ALICE, DEFAULT_USERS_INITIAL_BALANCE);
    // Submit program code into the system
    let code_id = system.submit_code(sessions::WASM_BINARY);

    // Create a remoting instance for the system
    // and set the block run mode to Next,
    // cause we don't receive any reply on `Exit` call
    let env = GtestEnv::new(system, DEFAULT_USER_ALICE.into());
    (env, code_id)
}

#[tokio::test]
async fn create_session_works() {
    let (env, program_code_id) = create_env();

    let config = SessionConfig {
        gas_to_delete_session: 10_000_000_000,
        minimum_session_duration_ms: 180_000,
        ms_per_block: 3_000,
    };

    let program = env
        .deploy::<sessions_client::SessionsClientProgram>(program_code_id, b"salt".to_vec())
        .new(config)
        .await
        .unwrap();

    let mut service_client = program.session();

    let service_listener = service_client.listener();
    let mut service_events = service_listener.listen().await.unwrap();

    let key = 10;

    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    service_client
        .create_session(signature_data, None)
        .await
        .unwrap();

    assert_eq!(
        service_events.next().await.unwrap(),
        (program.id(), SessionEvents::SessionCreated)
    );

    // check session in state
    let result = service_client
        .session_for_the_account(DEFAULT_USER_ALICE.into())
        .await
        .unwrap();

    assert!(result.is_some());

    // create session with signature
    let pair: Keypair = Keypair::generate_with(OsRng);
    let data_to_sign = SignatureData {
        key: DEFAULT_USER_ALICE.into(),
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

    service_client
        .create_session(signature_data, Some(raw_signature.to_vec()))
        .await
        .unwrap();

    assert_eq!(
        service_events.next().await.unwrap(),
        (program.id(), SessionEvents::SessionCreated)
    );

    // check session in state
    let result = service_client.session_for_the_account(key).await.unwrap();

    assert!(result.is_some());
}

#[tokio::test]
async fn create_session_failures() {
    let (env, program_code_id) = create_env();

    let config = SessionConfig {
        gas_to_delete_session: 10_000_000_000,
        minimum_session_duration_ms: 180_000,
        ms_per_block: 3_000,
    };

    let program = env
        .deploy::<sessions_client::SessionsClientProgram>(program_code_id, b"salt".to_vec())
        .new(config)
        .await
        .unwrap();

    let mut service_client = program.session();

    let service_listener = service_client.listener();
    let mut service_events = service_listener.listen().await.unwrap();

    // duration is less than minimum session duration
    let key = 10;

    let signature_data = SignatureData {
        key: key.into(),
        duration: 179_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client.create_session(signature_data, None).await;

    assert!(result.is_err());

    // duration id too long (more than 400 years)
    let signature_data = SignatureData {
        key: key.into(),
        duration: 12884901888000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client.create_session(signature_data, None).await;

    assert!(result.is_err());

    // there are no allowed actions
    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![],
    };

    let result = service_client.create_session(signature_data, None).await;

    assert!(result.is_err());

    // the session already exists
    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    service_client
        .create_session(signature_data, None)
        .await
        .unwrap();

    assert_eq!(
        service_events.next().await.unwrap(),
        (program.id(), SessionEvents::SessionCreated)
    );

    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    let result = service_client.create_session(signature_data, None).await;

    assert!(result.is_err())
}

#[tokio::test]
async fn delete_session_from_account_works() {
    let (env, program_code_id) = create_env();

    let config = SessionConfig {
        gas_to_delete_session: 10_000_000_000,
        minimum_session_duration_ms: 180_000,
        ms_per_block: 3_000,
    };

    let program = env
        .deploy::<sessions_client::SessionsClientProgram>(program_code_id, b"salt".to_vec())
        .new(config)
        .await
        .unwrap();

    let mut service_client = program.session();

    let service_listener = service_client.listener();
    let mut service_events = service_listener.listen().await.unwrap();

    // duration is less than minimum session duration
    let key = 10;

    let signature_data = SignatureData {
        key: key.into(),
        duration: 180_000,
        allowed_actions: vec![ActionsForSession::StartGame, ActionsForSession::Move],
    };

    service_client
        .create_session(signature_data, None)
        .await
        .unwrap();

    assert_eq!(
        service_events.next().await.unwrap(),
        (program.id(), SessionEvents::SessionCreated)
    );

    service_client.delete_session_from_account().await.unwrap();

    assert_eq!(
        service_events.next().await.unwrap(),
        (program.id(), SessionEvents::SessionDeleted)
    );

    // check state
    let result = service_client
        .session_for_the_account(DEFAULT_USER_ALICE.into())
        .await
        .unwrap();

    assert!(result.is_none());
}
