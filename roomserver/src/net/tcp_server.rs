use crate::mgr::room_mgr::RoomMgr;
use log::{error, info};
use std::sync::{Arc, Mutex};
use tools::cmd_code::RoomCode;
use tools::tcp::tcp_server;
use tools::tcp::TcpSender;
use tools::util::packet::Packet;

///处理客户端所有请求,每个客户端单独分配一个handler
pub struct TcpServerHandler {
    pub sender: Option<TcpSender>,
    pub rm: Arc<Mutex<RoomMgr>>,
}

unsafe impl Send for TcpServerHandler {}

unsafe impl Sync for TcpServerHandler {}

impl tools::tcp::Handler for TcpServerHandler {
    fn try_clone(&self) -> Self {
        let mut sender: Option<TcpSender> = None;
        if self.sender.is_some() {
            sender = Some(self.sender.as_ref().unwrap().clone());
        }
        TcpServerHandler {
            sender,
            rm: self.rm.clone(),
        }
    }

    ///客户端tcp链接激活事件
    fn on_open(&mut self, sender: TcpSender) {
        self.rm.lock().unwrap().set_sender(sender);
    }

    ///客户端tcp链接关闭事件
    fn on_close(&mut self) {
        info!("与tcp客户端断开连接");
    }

    ///客户端读取事件
    fn on_message(&mut self, mess: Vec<u8>) {
        let packet_array = Packet::build_array_from_server(mess);

        if let Err(e) = packet_array {
            error!("{:?}", e);
            return;
        }
        let packet_array = packet_array.unwrap();

        for packet in packet_array {
            //判断是否是房间服的命令，如果不是，则直接无视掉
            if packet.get_cmd() < RoomCode::Min as u32 || packet.get_cmd() > RoomCode::Max as u32 {
                error!("the cmd:{} is not belong roomserver!", packet.get_cmd());
                continue;
            }
            //异步处理业务逻辑
            async_std::task::spawn(handler_mess_s(self.rm.clone(), packet));
        }
    }
}

///处理客户端消息
async fn handler_mess_s(rm: Arc<Mutex<RoomMgr>>, packet: Packet) {
    let mut lock = rm.lock().unwrap();
    lock.invok(packet);
}

///创建新的tcp服务器,如果有问题，终端进程
pub fn new(address: &str, rm: Arc<Mutex<RoomMgr>>) {
    let sh = TcpServerHandler { sender: None, rm };
    let res = tcp_server::new(address, sh);
    if let Err(e) = res {
        error!("{:?}", e);
        std::process::abort();
    }
}
