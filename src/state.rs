use crate::types::{ClientId, DocumentId, ServerMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Clone)]
pub struct AppState {
    rooms: Arc<Mutex<HashMap<DocumentId, Vec<(ClientId, mpsc::UnboundedSender<ServerMessage>)>>>>,
}

impl AppState {
    pub fn new() -> Self {
        println!("[STATE] Creating new AppState");
        AppState {
            rooms: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn join_room(
        &self,
        document_id: DocumentId,
        client_id: ClientId,
        tx: mpsc::UnboundedSender<ServerMessage>,
    ) {
        let mut rooms = self.rooms.lock().await;
        let clients = rooms.entry(document_id.clone()).or_insert_with(Vec::new);
        clients.push((client_id.clone(), tx));

        println!(
            "[STATE] Client {} joined room '{}' (now {} clients)",
            client_id,
            document_id,
            clients.len()
        );
    }

    pub async fn leave_room(&self, document_id: &DocumentId, client_id: &ClientId) {
        let mut rooms = self.rooms.lock().await;

        if let Some(clients) = rooms.get_mut(document_id) {
            clients.retain(|(id, _)| id != client_id);

            println!(
                "[STATE] Client {} left room '{}' ({} clients remaining)",
                client_id,
                document_id,
                clients.len()
            );

            if clients.is_empty() {
                rooms.remove(document_id);
                println!(
                    "[STATE] Room '{}' is empty and has been deleted",
                    document_id
                );
            }
        }
    }

    pub async fn broadcast_to_room(&self, document_id: &DocumentId, message: ServerMessage) {
        let mut rooms = self.rooms.lock().await;

        if let Some(clients) = rooms.get_mut(document_id) {
            let mut failed_clients = Vec::new();

            println!(
                "[STATE] Broadcasting to room '{}' ({} clients)",
                document_id,
                clients.len()
            );

            for (client_id, tx) in clients.iter() {
                if let Err(e) = tx.send(message.clone()) {
                    println!(
                        "[STATE] Failed to send to client {} in room '{}': {:?}",
                        client_id, document_id, e
                    );
                    failed_clients.push(client_id.clone());
                }
            }

            if !failed_clients.is_empty() {
                println!(
                    "[STATE] Cleaning up {} failed clients from room '{}'",
                    failed_clients.len(),
                    document_id
                );

                clients.retain(|(id, _)| !failed_clients.contains(id));

                if clients.is_empty() {
                    rooms.remove(document_id);
                    println!(
                        "[STATE] Room '{}' is empty after cleanup and has been deleted",
                        document_id
                    );
                }
            }
        }
    }
}
