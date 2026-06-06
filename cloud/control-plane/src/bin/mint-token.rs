//! Mint an HS256 JWT for Houston Cloud (`HOUSTON_CLOUD_AUTH=jwt`).
//!
//! ```bash
//! cargo run -p houston-cloud-control-plane --bin houston-cloud-mint-token -- \
//!   --secret "$HOUSTON_CLOUD_JWT_SECRET" \
//!   --user-id 00000000-0000-0000-0000-000000000001 \
//!   --email dev@local
//! ```

use chrono::{Duration, Utc};
use houston_cloud_control_plane::auth::JwtClaims;
use jsonwebtoken::{encode, EncodingKey, Header};
use std::env;
use uuid::Uuid;

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let mut secret = None;
    let mut user_id = None;
    let mut email = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--secret" => secret = args.next(),
            "--user-id" => user_id = args.next(),
            "--email" => email = args.next(),
            "-h" | "--help" => {
                eprintln!(
                    "Usage: houston-cloud-mint-token --secret SECRET [--user-id UUID] [--email ADDR]"
                );
                return Ok(());
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }
    let secret = secret.ok_or_else(|| anyhow::anyhow!("--secret is required"))?;
    let user_id = user_id
        .map(|s| Uuid::parse_str(&s))
        .transpose()?
        .unwrap_or_else(|| Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let exp = (Utc::now() + Duration::days(365)).timestamp() as usize;
    let claims = JwtClaims {
        sub: user_id.to_string(),
        email,
        role: Some("authenticated".into()),
        exp,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    println!("{token}");
    Ok(())
}
