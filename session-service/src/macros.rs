/// This macro generates a session management system based on a provided enum of actions.
/// It creates the necessary data structures for sessions, including handling of signatures
/// and session storage, as well as a service for creating, deleting, and managing sessions.
///
/// # Example:
/// ```rust, ignore
/// #![no_std]
/// use sails_rs::prelude::*;
/// use session_service::*;
///
/// pub struct SessionsProgram{
///     session_storage: RefCell<Storage>,
/// }
///
/// #[sails_rs::program]
/// impl SessionsProgram {
///     pub async fn new(config: Config) -> Self {
///         Self {
///             session_storage: RefCell::new(Storage::new(config)),
///         }
///     }
///
///     pub fn session(&self) -> SessionService<'_> {
///         SessionService::new(&self.session_storage)
///     }
/// }
///
/// #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
/// #[codec(crate = sails_rs::scale_codec)]
/// #[scale_info(crate = sails_rs::scale_info)]
/// pub enum ActionsForSession {
///     StartGame,
///     Move,
///     Skip,
/// }
///
/// generate_session_system!(ActionsForSession);
#[macro_export]
macro_rules! generate_session_system {
    ($actions_enum:ident) => {
        use sails_rs::fmt::Debug;
        use sails_rs::{cell::RefCell, collections::HashMap, gstd::service};
        use $crate::{exec, msg, PublicKey};

        pub type SessionMap = HashMap<ActorId, SessionData>;

        pub struct Storage {
            sessions: SessionMap,
            config: Config,
        }

        impl Storage {
            pub fn new(config: Config) -> Self {
                Self {
                    sessions: HashMap::new(),
                    config,
                }
            }
        }

        #[derive(Debug, Default, Clone, Copy, Encode, Decode, TypeInfo, PartialEq, Eq)]
        #[codec(crate = sails_rs::scale_codec)]
        #[scale_info(crate = sails_rs::scale_info)]
        pub struct Config {
            pub gas_to_delete_session: u64,
            pub minimum_session_duration_ms: u64,
            pub ms_per_block: u64,
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

        #[event]
        #[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
        #[codec(crate = sails_rs::scale_codec)]
        #[scale_info(crate = sails_rs::scale_info)]
        pub enum SessionEvent {
            SessionCreated,
            SessionDeleted,
        }

        #[derive(Debug)]
        pub enum SessionError {
            BadSignature,
            BadPublicKey,
            VerificationFailed,
            DurationIsSmall,
            DurationIsLarge,
            ThereAreNoAllowedMessages,
            MessageOnlyForProgram,
            TooEarlyToDeleteSession,
            NoSession,
            AlreadyHaveActiveSession,
            SendMessageFailed,
            EmitEventFailed,
        }

        #[derive(Clone)]
        pub struct SessionService<'a> {
            storage: &'a RefCell<Storage>,
        }

        impl<'a> SessionService<'a> {
            pub fn new(storage: &'a RefCell<Storage>) -> Self {
                Self { storage }
            }

            fn get(&self) -> core::cell::Ref<'_, Storage> {
                self.storage.borrow()
            }

            fn get_mut(&self) -> core::cell::RefMut<'_, Storage> {
                self.storage.borrow_mut()
            }
        }

        #[sails_rs::service(events = SessionEvent)]
        impl<'a> SessionService<'a> {
            #[export(unwrap_result)]
            pub fn create_session(
                &mut self,
                signature_data: SignatureData,
                signature: Option<Vec<u8>>,
            ) -> Result<(), SessionError> {
                let mut storage = self.get_mut();

                if signature_data.duration < storage.config.minimum_session_duration_ms {
                    return Err(SessionError::DurationIsSmall);
                }

                let msg_source = msg::source();
                let block_timestamp = exec::block_timestamp();
                let block_height = exec::block_height();

                let expires = block_timestamp + signature_data.duration;

                let number_of_blocks = u32::try_from(
                    signature_data
                        .duration
                        .div_ceil(storage.config.ms_per_block),
                )
                .map_err(|_| SessionError::DurationIsLarge)?;

                if signature_data.allowed_actions.is_empty() {
                    return Err(SessionError::ThereAreNoAllowedMessages);
                }

                let account = match signature {
                    Some(sig_bytes) => {
                        check_if_session_exists(&storage.sessions, &signature_data.key)?;
                        let pub_key: [u8; 32] = (signature_data.key).into();
                        let message = SignatureData {
                            key: msg_source,
                            duration: signature_data.duration,
                            allowed_actions: signature_data.allowed_actions.clone(),
                        }
                        .encode();

                        let mut complete_message = Vec::with_capacity(
                            b"<Bytes>".len() + message.len() + b"</Bytes>".len(),
                        );
                        complete_message.extend_from_slice(b"<Bytes>");
                        complete_message.extend_from_slice(&message);
                        complete_message.extend_from_slice(b"</Bytes>");

                        verify(&sig_bytes, complete_message, pub_key)?;
                        storage
                            .sessions
                            .entry(signature_data.key)
                            .insert(SessionData {
                                key: msg_source,
                                expires,
                                allowed_actions: signature_data.allowed_actions,
                                expires_at_block: block_height + number_of_blocks,
                            });
                        signature_data.key
                    }
                    None => {
                        check_if_session_exists(&storage.sessions, &msg_source)?;
                        storage.sessions.entry(msg_source).insert(SessionData {
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
                    storage.config.gas_to_delete_session,
                    0,
                    number_of_blocks,
                )
                .map_err(|_| SessionError::SendMessageFailed)?;

                self.emit_event(SessionEvent::SessionCreated)
                    .map_err(|_| SessionError::EmitEventFailed)?;
                Ok(())
            }

            #[export(unwrap_result)]
            pub fn delete_session_from_program(
                &mut self,
                session_for_account: ActorId,
            ) -> Result<(), SessionError> {
                if msg::source() != exec::program_id() {
                    return Err(SessionError::MessageOnlyForProgram);
                }

                let mut storage = self.get_mut();

                if let Some(session) = storage.sessions.remove(&session_for_account) {
                    if session.expires_at_block > exec::block_height() {
                        return Err(SessionError::TooEarlyToDeleteSession);
                    }
                }
                self.emit_event(SessionEvent::SessionDeleted)
                    .map_err(|_| SessionError::EmitEventFailed)?;
                Ok(())
            }

            #[export(unwrap_result)]
            pub fn delete_session_from_account(&mut self) -> Result<(), SessionError> {
                let mut storage = self.get_mut();
                if storage.sessions.remove(&msg::source()).is_none() {
                    return Err(SessionError::NoSession);
                }

                self.emit_event(SessionEvent::SessionDeleted)
                    .map_err(|_| SessionError::EmitEventFailed)?;
                Ok(())
            }

            #[export]
            pub fn sessions(&self) -> Vec<(ActorId, SessionData)> {
                self.get()
                    .sessions
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect()
            }

            #[export]
            pub fn session_for_the_account(&self, account: ActorId) -> Option<SessionData> {
                self.get().sessions.get(&account).cloned()
            }
        }

        fn verify<P: AsRef<[u8]>, M: AsRef<[u8]>>(
            signature: &[u8],
            message: M,
            pubkey: P,
        ) -> Result<(), SessionError> {
            let signature =
                Signature::from_bytes(signature).map_err(|_| SessionError::BadSignature)?;
            let pub_key =
                PublicKey::from_bytes(pubkey.as_ref()).map_err(|_| SessionError::BadPublicKey)?;
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
