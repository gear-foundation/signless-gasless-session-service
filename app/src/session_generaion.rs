#[macro_export]
macro_rules! generate_session_system {
    ($actions_enum:ident) => {
        use sails_rs::{collections::HashMap, gstd::service};
        use gstd::{exec, msg, ext, format};
        use schnorrkel::PublicKey;
        use sails_rs::fmt::Debug;

        #[derive(Default)]
        pub struct Storage(());

        impl Storage {
            pub fn get_session_map() -> &'static SessionMap {
                unsafe { STORAGE.as_ref().expect("Storage is not initialized") }
            }
        }

        static mut STORAGE: Option<SessionMap> = None;
        static mut CONFIG: Option<Config> = None;

        #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
        #[codec(crate = sails_rs::scale_codec)]
        #[scale_info(crate = sails_rs::scale_info)]
        pub enum Event {
            SessionCreated,
            SessionDeleted,
        }

        #[derive(Clone)]
        pub struct SessionService(());

        impl SessionService {
            pub fn init(config: Config) -> Self {
                unsafe {
                    STORAGE = Some(HashMap::new());
                    CONFIG = Some(config);
                }
                Self(())
            }

            pub fn as_mut(&mut self) -> &'static mut SessionMap {
                unsafe { STORAGE.as_mut().expect("Storage is not initialized") }
            }

            pub fn as_ref(&self) -> &'static SessionMap {
                unsafe { STORAGE.as_ref().expect("Storage is not initialized") }
            }

            pub fn config(&self) -> &'static Config {
                unsafe { CONFIG.as_ref().expect("Config is not initialized") }
            }
        }

        #[service(events = Event)]
        impl SessionService {
            pub fn new() -> Self {
                Self(())
            }

            pub fn create_session(
                &mut self,
                signature_data: SignatureData,
                signature: Option<Vec<u8>>,
            ) {
                let sessions = self.as_mut();
                let config = self.config();
                let event = panicking(|| {
                    create_session(sessions, config, signature_data, signature)
                });
                self.notify_on(event.clone()).expect("Notification Error");
            }

            pub fn delete_session_from_program(&mut self, session_for_account: ActorId) {
                let sessions = self.as_mut();
                let event = panicking(|| {
                    delete_session_from_program(sessions, session_for_account)
                });
                self.notify_on(event.clone()).expect("Notification Error");
            }

            pub fn delete_session_from_account(&mut self) {
                let sessions = self.as_mut();
                let event = panicking(|| delete_session_from_account(sessions));
                self.notify_on(event.clone()).expect("Notification Error");
            }

            pub fn sessions(&self) -> Vec<(ActorId, SessionData)> {
                self.as_ref().clone().into_iter().collect()
            }

            pub fn session_for_the_account(&self, account: ActorId) -> Option<SessionData> {
                self.as_ref().get(&account).cloned()
            }
        }

        pub type SessionMap = HashMap<ActorId, SessionData>;

        #[derive(Debug, Default, Clone, Copy, Encode, Decode, TypeInfo, PartialEq, Eq)]
        #[codec(crate = sails_rs::scale_codec)]
        #[scale_info(crate = sails_rs::scale_info)]
        pub struct Config {
            pub gas_to_delete_session: u64,
            pub minimum_session_duration_ms: u64,
            pub s_per_block: u64,
        }

        #[derive(Debug, Clone, Encode, Decode, TypeInfo)]
        #[codec(crate = sails_rs::scale_codec)]
        #[scale_info(crate = sails_rs::scale_info)]
        pub enum SessionError {
            BadSignature,
            BadPublicKey,
            VerificationFailed,
            DurationIsSmall,
            ThereAreNoAllowedMessages,
            MessageOnlyForProgram,
            TooEarlyToDeleteSession,
            NoSession,
            AlreadyHaveActiveSession,
        }

        // This structure is for creating a gaming session, which allows players to predefine certain actions for an account
        // that will play the game on their behalf for a certain period of time.
        #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
        #[codec(crate = sails_rs::scale_codec)]
        #[scale_info(crate = sails_rs::scale_info)]
        pub struct SessionData {
            // The address of the player who will play on behalf of the user
            pub key: ActorId,
            // Until what time the session is valid
            pub expires: u64,
            // What messages are allowed to be sent by the account (key)
            pub allowed_actions: Vec<$actions_enum>,
            pub expires_at_block: u32,
        }

        #[derive(Encode, Decode, TypeInfo)]
        #[codec(crate = sails_rs::scale_codec)]
        #[scale_info(crate = sails_rs::scale_info)]
        pub struct SignatureData {
            pub key: ActorId,
            pub duration: u64,
            pub allowed_actions: Vec<$actions_enum>,
        }

        pub fn panicking<T, E: Debug, F: FnOnce() -> Result<T, E>>(f: F) -> T {
            match f() {
                Ok(v) => v,
                Err(e) => panic(e),
            }
        }

        pub fn panic(err: impl Debug) -> ! {
            ext::panic(&format!("{err:?}"))
        }

        pub fn create_session(
            sessions: &mut SessionMap,
            config: &Config,
            signature_data: SignatureData,
            signature: Option<Vec<u8>>,
        ) -> Result<Event, SessionError> {
            if signature_data.duration < config.minimum_session_duration_ms {
                return Err(SessionError::DurationIsSmall);
            }

            let msg_source = msg::source();
            let block_timestamp = exec::block_timestamp();
            let block_height = exec::block_height();

            let expires = block_timestamp + signature_data.duration;

            let number_of_blocks = u32::try_from(signature_data.duration.div_ceil(config.s_per_block * 1_000))
                .expect("Duration is too large");

            if signature_data.allowed_actions.is_empty() {
                return Err(SessionError::ThereAreNoAllowedMessages);
            }

            let account = match signature {
                Some(sig_bytes) => {
                    check_if_session_exists(sessions, &signature_data.key)?;
                    let pub_key: [u8; 32] = (signature_data.key).into();
                    let mut prefix = b"<Bytes>".to_vec();
                    let mut message = SignatureData {
                        key: msg_source,
                        duration: signature_data.duration,
                        allowed_actions: signature_data.allowed_actions.clone(),
                    }
                    .encode();
                    let mut postfix = b"</Bytes>".to_vec();
                    prefix.append(&mut message);
                    prefix.append(&mut postfix);

                    verify(&sig_bytes, prefix, pub_key)?;
                    sessions.entry(signature_data.key).insert(SessionData {
                        key: msg_source,
                        expires,
                        allowed_actions: signature_data.allowed_actions,
                        expires_at_block: block_height + number_of_blocks,
                    });
                    signature_data.key
                }
                None => {
                    check_if_session_exists(sessions, &msg_source)?;

                    sessions.entry(msg_source).insert(SessionData {
                        key: signature_data.key,
                        expires,
                        allowed_actions: signature_data.allowed_actions,
                        expires_at_block: block_height + number_of_blocks,
                    });
                    msg_source
                }
            };

            let request = [
                "Session".encode(),
                "DeleteSessionFromProgram".to_string().encode(),
                (account).encode(),
            ]
            .concat();

            msg::send_bytes_with_gas_delayed(
                exec::program_id(),
                request,
                config.gas_to_delete_session,
                0,
                number_of_blocks,
            )
            .expect("Error in sending message");

            Ok(Event::SessionCreated)
        }

        pub fn delete_session_from_program(
            sessions: &mut SessionMap,
            session_for_account: ActorId,
        ) -> Result<Event, SessionError> {
            if msg::source() != exec::program_id() {
                return Err(SessionError::MessageOnlyForProgram);
            }

            if let Some(session) = sessions.remove(&session_for_account) {
                if session.expires_at_block > exec::block_height() {
                    return Err(SessionError::TooEarlyToDeleteSession);
                }
            }
            Ok(Event::SessionDeleted)
        }

        pub fn delete_session_from_account(sessions: &mut SessionMap) -> Result<Event, SessionError> {
            if sessions.remove(&msg::source()).is_none() {
                return Err(SessionError::NoSession);
            }
            Ok(Event::SessionDeleted)
        }

        fn verify<P: AsRef<[u8]>, M: AsRef<[u8]>>(
            signature: &[u8],
            message: M,
            pubkey: P,
        ) -> Result<(), SessionError> {
            let signature =
                schnorrkel::Signature::from_bytes(signature).map_err(|_| SessionError::BadSignature)?;
            let pub_key = PublicKey::from_bytes(pubkey.as_ref()).map_err(|_| SessionError::BadPublicKey)?;
            pub_key
                .verify_simple(b"substrate", message.as_ref(), &signature)
                .map(|_| ())
                .map_err(|_| SessionError::VerificationFailed)
        }

        fn check_if_session_exists(
            session_map: &HashMap<ActorId, SessionData>,
            account: &ActorId,
        ) -> Result<(), SessionError> {
            if let Some(SessionData {
                key: _,
                expires: _,
                allowed_actions: _,
                expires_at_block,
            }) = session_map.get(account)
            {
                if *expires_at_block > exec::block_height() {
                    return Err(SessionError::AlreadyHaveActiveSession);
                }
            }
            Ok(())
        }
    };
}
