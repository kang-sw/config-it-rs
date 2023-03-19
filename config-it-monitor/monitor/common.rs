pub mod util {
    use anyhow::anyhow;
    use rpc_it::RetrievePayload;

    pub trait JsonPayload {
        fn json_payload<'de, D: serde::de::Deserialize<'de>>(
            &'de self,
        ) -> Result<D, serde_json::Error>;
    }

    impl<T> JsonPayload for T
    where
        T: RetrievePayload,
    {
        fn json_payload<'de, D: serde::de::Deserialize<'de>>(
            &'de self,
        ) -> Result<D, serde_json::Error> {
            serde_json::from_slice(self.payload())
        }
    }

    pub async fn remote_call<T>(
        rpc: &rpc_it::Handle,
        method: &str,
        param: &impl serde::Serialize,
    ) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let param = serde_json::to_vec(param).unwrap();
        let reply = rpc.request(method, [&param]).await?;
        rpc.flush().await?;

        let reply = reply
            .await
            .ok_or_else(|| anyhow!("failed to acquire reply"))?
            .result()?;

        Ok(serde_json::from_slice(reply.payload())?)
    }

    pub async fn reply_as(
        req: rpc_it::Request,
        result: &impl serde::Serialize,
    ) -> anyhow::Result<usize> {
        Ok(req.reply([&serde_json::to_vec(result)?]).await?)
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize
)]
pub enum AuthLevel {
    /// May participate in chatting
    Chat = 0,

    /// May inspect logging output
    InspectLog = 1,

    /// May access to configuration storages.
    InspectConfig = 2,

    /// May modify configuration storages.
    ModifyConfig = 3,

    /// Can do anything
    Admin = 4,
}

pub mod handshake {
    //!
    //! Handshake protocols right after establishing websocket connection
    //!
    //! 0. C->S send 'hello' -> reply 'world'
    //! 1. C->S request for system info
    //! ...
    //! FINAL. C->S request for login
    //! -> start primary session
    //!

    use base64::engine::general_purpose::STANDARD;
    use base64_serde::base64_serde_type;

    base64_serde_type!(Base64Standard, STANDARD);

    use sha2::Digest;

    macro_rules! declare_route {
        ($ident:ident) => {
            pub const $ident: &str = concat!("handshake::", stringify!($ident));
        };
    }

    // TODO: Implement authentication
    declare_route!(HELLO);
    declare_route!(SYSTEM_INTRODUCE);
    declare_route!(LOGIN);

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct SystemIntroduce {
        pub system_name: String,
        pub monitor_version: String,
        pub system_description: String,

        pub desktop_name: String,
        pub num_cores: usize,
        pub epoch_utc: u64,
    }

    #[derive(derive_getters::Getters, serde::Serialize, serde::Deserialize)]
    pub struct LoginRequest {
        id: String,

        #[serde(with = "Base64Standard")]
        passwd_hash: Vec<u8>,
    }

    impl LoginRequest {
        pub fn new(id: String, passwd: &str) -> Self {
            Self {
                id,
                passwd_hash: {
                    let mut hasher = sha2::Sha512::new();
                    hasher.update(passwd);
                    let s: [u8; 64] = hasher.finalize().into();
                    s.to_vec()
                },
            }
        }
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct LoginResult {
        pub auth_level: super::AuthLevel,
    }
}
