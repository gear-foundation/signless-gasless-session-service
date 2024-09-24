## Adding Dependencies
To use session-service in your project, add the following dependency to your Cargo.toml:

```toml
[dependencies]
session-service = { git = "https://github.com/gear-foundation/signless-gasless-session-service.git" }
```

## Usage
```rust
#![no_std]
use sails_rs::prelude::*;
use session_service::*;

pub struct Program(());

#[program]
impl Program {
    pub async fn new(config: Config) -> Self {
        // Initialize your services
        ...
        SessionService::init(config);
        Self(())
    }

    // Define your services
    ...

    pub fn session(&self) -> SessionService {
        SessionService::new()
    }
}

// Define your custom session actions using an enum
#[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
#[codec(crate = sails_rs::scale_codec)]
#[scale_info(crate = sails_rs::scale_info)]
pub enum ActionsForSession {
    StartGame,
    Move,
    Skip,
}

// Use the macro to generate session management structures and logic
generate_session_system!(ActionsForSession);

```