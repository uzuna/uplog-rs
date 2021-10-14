use crate::{writer::RecordWriter, Session, Storage};
use actix::prelude::*;
use actix_web_actors::ws;
use log::{debug, error, info, warn};
use uplog::Record;
use uuid::Uuid;

#[derive(Message)]
#[rtype(result = "()")]
pub struct StorageRequest {
    addr: Recipient<StorageResponse>,
    self_id: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub enum StorageResponse {
    Accept(Recipient<SessionCommand>),
    Error(String),
}

#[derive(Message)]
#[rtype(result = "()")]
pub enum SessionCommand {
    Record(uplog::Record),
    Close,
}

pub struct StorageActor {
    storage: Storage,
}

impl StorageActor {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    pub fn get_session(&self, uuid: Uuid) -> std::io::Result<Session> {
        self.storage.create_session(uuid.to_string().as_str())
    }
}

impl Actor for StorageActor {
    type Context = Context<Self>;
}

impl Handler<StorageRequest> for StorageActor {
    type Result = ();

    fn handle(&mut self, msg: StorageRequest, _ctx: &mut Self::Context) -> Self::Result {
        let res = match self.get_session(msg.self_id) {
            Ok(session) => {
                let addr = SessionActor::new(session).start().recipient();
                StorageResponse::Accept(addr)
            }
            Err(e) => StorageResponse::Error(format!("failed to create {}", e)),
        };
        msg.addr.do_send(res).unwrap();
    }
}

struct SessionActor {
    session: Session,
}

impl SessionActor {
    fn new(session: Session) -> Self {
        Self { session }
    }
}

impl Actor for SessionActor {
    type Context = Context<Self>;
}

impl Handler<SessionCommand> for SessionActor {
    type Result = ();

    fn handle(&mut self, msg: SessionCommand, ctx: &mut Self::Context) -> Self::Result {
        use SessionCommand::*;
        match msg {
            Record(record) => {
                self.session
                    .push(&record)
                    .map_err(|e| error!("failed to write {}", e))
                    .ok();
            }
            Close => ctx.stop(),
        }
    }
}

pub struct WsConn {
    id: uuid::Uuid,
    storage_addr: Recipient<StorageRequest>,
    session_addr: Option<Recipient<SessionCommand>>,
}

impl WsConn {
    pub fn new(id: Uuid, storage_addr: Recipient<StorageRequest>) -> Self {
        Self {
            id,
            storage_addr,
            session_addr: None,
        }
    }
}

impl Actor for WsConn {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.storage_addr
            .send(StorageRequest {
                addr: ctx.address().recipient(),
                self_id: self.id,
            })
            .into_actor(self)
            .then(|res, _, ctx| {
                match res {
                    Ok(_res) => (),
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        // 即座に送信して終了する(待たない)ためdo_send
        self.session_addr.as_ref().and_then(|r| {
            r.do_send(SessionCommand::Close)
                .map_err(|e| {
                    warn!("failed to send close signal [{}], cause {}", self.id, e);
                })
                .ok()
        });
        Running::Stop
    }
}

impl Handler<StorageResponse> for WsConn {
    type Result = ();

    fn handle(&mut self, msg: StorageResponse, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            StorageResponse::Accept(a) => self.session_addr = Some(a),
            StorageResponse::Error(e) => {
                error!("failed to create session {}", e);
                ctx.stop();
            }
        };
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConn {
    fn handle(&mut self, item: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match item {
            Ok(ws::Message::Binary(bin)) => {
                let iter = serde_cbor::Deserializer::from_slice(&bin).into_iter::<Record>();
                for v in iter {
                    match v {
                        Ok(v) => {
                            debug!("accept data [{}] {}", self.id, v);
                            self.session_addr.as_ref().and_then(|r| {
                                r.do_send(SessionCommand::Record(v))
                                    .map_err(|e| error!("session write error [{}] {:?}", self.id, e))
                                    .ok()
                            });
                        }
                        Err(e) => {
                            warn!("format error [{}] {:?}", self.id, e);
                        }
                    };
                }
            }
            Ok(ws::Message::Close(reason)) => {
                info!("close by client [{}] {:?}", self.id, reason);
                ctx.stop();
            }
            Ok(_msg) => {}
            Err(e) => {
                warn!("connection error [{}] {:?}", self.id, e);
                ctx.stop()
            }
        }
    }
}
