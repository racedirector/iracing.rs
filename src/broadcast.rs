const BROADCAST_MESSAGE_NAME: &str = r"IRSDK_BROADCASTMSG";

#[derive(Debug, Copy, Clone)]
pub struct Broadcast {
    message_id: u32,
}

impl Broadcast {
    pub fn new() -> Broadcast {
        Broadcast { message_id: 1 }
    }

    pub fn reload_textures(&self, car_index: u8) {
        println!("Reloading textures for car index {}.", car_index);
        self.send_message();
    }
    pub fn reload_all_textures(&self) {
        // SendNotifyMessageW(0xFFFF, self.message_id)
    }

    fn send_message(&self) {
        println!("Sending message!")
    }
}
