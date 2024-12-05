use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct DeezerUserResponse {
    pub id: usize,
    pub name: String,
    pub country: String,
    pub email: String,
}

#[derive(Deserialize, Debug)]
pub struct DeezerJwtResponse {
    pub jwt: String,
}
