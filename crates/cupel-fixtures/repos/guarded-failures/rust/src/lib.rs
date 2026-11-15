// ABOUTME: Which servers of a configuration may vote, and parsing a server from its ID.
// ABOUTME: The fixture's tests check it by panicking under conditions (ADR-221).

pub struct Server {
    pub id: String,
    pub voter: bool,
}

pub fn has_vote(servers: &[Server], id: &str) -> bool {
    servers.iter().any(|server| server.id == id && server.voter)
}

pub fn parse(id: &str) -> Result<Server, String> {
    if id.is_empty() {
        Err("empty ID".to_owned())
    } else {
        Ok(Server {
            id: id.to_owned(),
            voter: true,
        })
    }
}
