use super::*;
use crate::entity::member::Member;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Team {
    id: u32,
    members: HashMap<u32, Member>,
}

impl Team {
    ///判断是否存在该成员
    pub fn is_exist_member(&self, id: &u32) -> bool {
        let result = self.members.contains_key(id);
        result
    }

    ///添加成员
    pub fn add_member(&mut self, member: Member) {
        self.members.insert(member.get_user_id(), member);
    }

    ///获取成员的可变指针
    pub fn get_member_mut(&mut self, user_id: &u32) -> Option<&mut Member> {
        self.members.get_mut(&user_id)
    }
}
