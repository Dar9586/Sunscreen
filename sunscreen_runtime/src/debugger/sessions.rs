use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::{DebugInfo, SealData};

use sunscreen_compiler_common::CompilationResult;
use sunscreen_fhe_program::Operation;

use seal_fhe::SecretKey;
use sunscreen_zkp_backend::CompiledZkpProgram;

// Global data structure storing session information
static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();

pub fn get_sessions() -> &'static Mutex<HashMap<String, Session>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/**
 * Determines if the session being debugged is for an FHE or ZKP program.
 */
pub enum Session {
    BfvSession(BfvSession),
}

impl From<BfvSession> for Session {
    fn from(value: BfvSession) -> Self {
        Self::BfvSession(value)
    }
}

impl Session {
    pub fn unwrap_bfv_session(&self) -> &BfvSession {
        match self {
            Self::BfvSession(s) => s,
            _ => panic!("Expected BfvSession"),
        }
    }

    pub fn unwrap_bfv_session_mut(&mut self) -> &mut BfvSession {
        match self {
            Self::BfvSession(s) => s,
            _ => panic!("Expected BfvSession"),
        }
    }
}

/**
 * Stores the relevant information for debugging an FHE program.
 */
pub struct BfvSession {
    /**
     * The compilation graph used to execute the program.
     */
    pub graph: CompilationResult<Operation>,
    /**
     * The values of operands in the compilation graph.
     */
    pub program_data: Vec<Option<SealData>>,
    /**
     * Used for decryption of ciphertexts for visualization.
     */
    pub secret_key: SecretKey,
}
impl BfvSession {
    /**
     * Constructs a new `FheDebugInfo`.
     */
    pub fn new(graph: &CompilationResult<Operation>, secret_key: &SecretKey) -> Self {
        Self {
            graph: graph.clone(),
            // don't need a hashmap; if you don't encounter in the right order, it's all initialize das None so you
            // can go back later and fill it in
            program_data: vec![None; graph.node_count()],

            secret_key: secret_key.clone(),
        }
    }
}

/**
 * Lazily starts a webserver at `http://localhost:8080`.
 */
pub async fn start_web_server() -> std::io::Result<()> {
    let url = "http://localhost:8080";
    match reqwest::get(url).await {
        Ok(_response) => Ok(()),
        Err(_e) => {
            HttpServer::new(move || App::new().service(get_graph_data))
                .bind(("127.0.0.1", 8080))?
                .run()
                .await
        }
    }
}

/**
 * Gets the graph data of a function.
 */
#[get("/")]
async fn get_graph_data(session: String) -> impl Responder {
    // TODO: add error catching
    let curr_session = get_sessions()
        .lock()
        .unwrap()
        .get(&session)
        .unwrap()
        .unwrap_bfv_session();

    HttpResponse::Ok().body(curr_session)
}
