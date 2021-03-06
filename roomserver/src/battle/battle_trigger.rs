use crate::battle::battle::BattleData;
use crate::battle::battle_buff::Buff;
use crate::battle::battle_enum::buff_type::{
    ATTACKED_ADD_ENERGY, CAN_NOT_MOVED, CHANGE_SKILL, DEFENSE_NEAR_MOVE_SKILL_DAMAGE, LOCKED,
    LOCK_SKILLS, TRANSFORM_BUFF, TRAPS, TRAP_ADD_BUFF, TRAP_SKILL_DAMAGE,
};
use crate::battle::battle_enum::skill_judge_type::{LIMIT_ROUND_TIMES, LIMIT_TURN_TIMES};
use crate::battle::battle_enum::skill_type::WATER_TURRET;
use crate::battle::battle_enum::EffectType::AddSkill;
use crate::battle::battle_enum::{ActionType, EffectType, TRIGGER_SCOPE_NEAR};
use crate::battle::battle_skill::Skill;
use crate::room::character::BattleCharacter;
use crate::TEMPLATES;
use log::{error, warn};
use std::borrow::BorrowMut;
use std::collections::HashMap;
use tools::protos::base::{ActionUnitPt, EffectPt, TargetPt, TriggerEffectPt};

///触发事件trait
pub trait TriggerEvent {
    ///翻开地图块时候触发
    unsafe fn open_map_cell_buff_trigger(
        &mut self,
        user_id: u32,
        au: &mut ActionUnitPt,
        is_pair: bool,
    ) -> anyhow::Result<Option<ActionUnitPt>>;

    ///被移动前触发buff
    fn before_moved_trigger(&self, from_user: u32, target_user: u32) -> anyhow::Result<()>;

    ///移动位置后触发事件
    unsafe fn after_move_trigger(
        &mut self,
        battle_cter: &mut BattleCharacter,
        index: isize,
        is_change_index_both: bool,
        battle_cters: &mut HashMap<u32, BattleCharacter>,
    ) -> Vec<ActionUnitPt>;

    ///使用技能后触发
    fn after_use_skill_trigger(
        &mut self,
        user_id: u32,
        skill_id: u32,
        is_item: bool,
        au: &mut ActionUnitPt,
    );

    fn before_use_skill_trigger(&mut self, user_id: u32) -> anyhow::Result<()>;

    ///受到攻击后触发
    fn attacked_buffs_trigger(&mut self, user_id: u32, target_pt: &mut TargetPt);

    ///地图刷新时候触发buff
    fn before_map_refresh_buff_trigger(&mut self);

    ///buff失效时候触发
    fn buff_lost_trigger(&mut self, user_id: u32, buff_id: u32);
}

impl BattleData {
    ///触发陷阱
    pub fn trigger_trap(
        &mut self,
        battle_cter: &mut BattleCharacter,
        index: usize,
    ) -> Option<Vec<ActionUnitPt>> {
        let map_cell = self.tile_map.map_cells.get_mut(index);
        if let None = map_cell {
            warn!("map do not has this map_cell!index:{}", index);
            return None;
        }
        let mut au_v = Vec::new();
        let turn_index = self.next_turn_index;
        let user_id = battle_cter.get_user_id();
        let map_cell = map_cell.unwrap();
        for buff in map_cell.buffs.clone().values() {
            let buff_id = buff.id;
            //先判断是否是陷阱类buff
            if !TRAPS.contains(&buff.id) {
                continue;
            }
            let mut target_pt = None;
            //判断是否是上buff的陷阱
            if TRAP_ADD_BUFF.contains(&buff.id) {
                let buff_id = buff.buff_temp.par1;
                let buff_temp = TEMPLATES.get_buff_temp_mgr_ref().get_temp(&buff_id);
                if let Err(e) = buff_temp {
                    warn!("{:?}", e);
                    continue;
                }
                let buff_temp = buff_temp.unwrap();
                let buff_add = Buff::new(buff_temp, Some(turn_index), None, None);
                battle_cter.battle_buffs.buffs.insert(buff.id, buff_add);

                let mut target_pt_tmp = TargetPt::new();
                target_pt_tmp
                    .target_value
                    .push(battle_cter.get_map_cell_index() as u32);
                target_pt_tmp.add_buffs.push(buff_id);
                target_pt = Some(target_pt_tmp);
            } else if TRAP_SKILL_DAMAGE.contains(&buff.id) {
                //造成技能伤害的陷阱
                let skill_damage = buff.buff_temp.par1 as i16;
                unsafe {
                    let target_pt_tmp = self.deduct_hp(0, user_id, Some(skill_damage), true);
                    if let Err(e) = target_pt_tmp {
                        error!("{:?}", e);
                        continue;
                    }
                    let target_pt_tmp = target_pt_tmp.unwrap();
                    target_pt = Some(target_pt_tmp);
                }
            }

            if target_pt.is_none() {
                continue;
            }
            if buff.from_user.is_none() {
                continue;
            }
            let mut target_pt = target_pt.unwrap();
            let mut aup = ActionUnitPt::new();
            unsafe {
                let lost_buff = self.consume_buff(buff_id, None, Some(index), false);
                if let Some(lost_buff) = lost_buff {
                    target_pt.lost_buffs.push(lost_buff);
                }
            }
            aup.from_user = buff.from_user.unwrap();
            aup.action_type = ActionType::Buff as u32;
            aup.action_value.push(buff.id);
            aup.targets.push(target_pt);
            au_v.push(aup);
        }
        Some(au_v)
    }
}

impl TriggerEvent for BattleData {
    unsafe fn open_map_cell_buff_trigger(
        &mut self,
        user_id: u32,
        au: &mut ActionUnitPt,
        is_pair: bool,
    ) -> anyhow::Result<Option<ActionUnitPt>> {
        let battle_cters = self.battle_cter.borrow_mut() as *mut HashMap<u32, BattleCharacter>;
        let battle_cter = battle_cters.as_mut().unwrap().get_mut(&user_id).unwrap();
        let index = battle_cter.get_map_cell_index();
        //匹配玩家身上的
        self.trigger_open_map_cell_buff(None, user_id, battle_cters, au, is_pair);
        //匹配地图块的
        self.trigger_open_map_cell_buff(Some(index), user_id, battle_cters, au, is_pair);
        Ok(None)
    }

    fn before_moved_trigger(&self, from_user: u32, target_user: u32) -> anyhow::Result<()> {
        //先判断目标位置的角色是否有不动泰山被动技能
        let target_cter = self.get_battle_cter(Some(target_user), true).unwrap();
        if target_cter.battle_buffs.buffs.contains_key(&CAN_NOT_MOVED) && from_user != target_user {
            anyhow::bail!(
                "this cter can not be move!cter_id:{},buff_id:{}",
                target_user,
                CAN_NOT_MOVED
            )
        }
        Ok(())
    }

    ///移动后触发事件，大多数为buff
    unsafe fn after_move_trigger(
        &mut self,
        battle_cter: &mut BattleCharacter,
        index: isize,
        is_change_index_both: bool,
        battle_cters: &mut HashMap<u32, BattleCharacter>,
    ) -> Vec<ActionUnitPt> {
        let mut v = Vec::new();

        //触发陷阱
        self.trigger_trap(battle_cter, index as usize);
        //触发范围
        for other_cter in battle_cters.values_mut() {
            if other_cter.is_died() {
                continue;
            }
            let cter_index = other_cter.get_map_cell_index() as isize;

            //踩到别人到范围
            for buff in other_cter.battle_buffs.buffs.values_mut() {
                if !DEFENSE_NEAR_MOVE_SKILL_DAMAGE.contains(&buff.id) {
                    continue;
                }
                if is_change_index_both {
                    continue;
                }

                for scope_index in TRIGGER_SCOPE_NEAR.iter() {
                    let res = cter_index + scope_index;
                    if index != res {
                        continue;
                    }
                    let target_pt = self.deduct_hp(
                        other_cter.base_attr.user_id,
                        battle_cter.base_attr.user_id,
                        Some(buff.buff_temp.par1 as i16),
                        true,
                    );
                    match target_pt {
                        Ok(target_pt) => {
                            let mut other_aupt = ActionUnitPt::new();
                            other_aupt.from_user = other_cter.base_attr.user_id;
                            other_aupt.action_type = ActionType::Buff as u32;
                            other_aupt.action_value.push(buff.id);
                            other_aupt.targets.push(target_pt);
                            v.push(other_aupt);
                            break;
                        }
                        Err(e) => error!("{:?}", e),
                    }
                }
                if battle_cter.is_died() {
                    break;
                }
            }
            //别人进入自己的范围触发
            //现在没有种buff，先注释代码
            // if battle_cter.get_user_id() == other_cter.get_user_id() {
            //     continue;
            // }
            // for buff in battle_cter.buffs.values_mut() {
            //     if !DEFENSE_NEAR_MOVE_SKILL_DAMAGE.contains(&buff.id) {
            //         continue;
            //     }
            //     let mut need_rank = true;
            //     for scope_index in TRIGGER_SCOPE_NEAR.iter() {
            //         let res = index + scope_index;
            //         if cter_index != res {
            //             continue;
            //         }
            //         let target_pt = self.deduct_hp(
            //             battle_cter.get_user_id(),
            //             other_cter.get_user_id(),
            //             Some(buff.buff_temp.par1 as i32),
            //             need_rank,
            //         );
            //         match target_pt {
            //             Ok(target_pt) => {
            //                 let mut self_aupt = ActionUnitPt::new();
            //                 self_aupt.from_user = user_id;
            //                 self_aupt.action_type = ActionType::Buff as u32;
            //                 self_aupt.action_value.push(buff.id);
            //                 self_aupt.targets.push(target_pt);
            //                 v.push(self_aupt);
            //                 break;
            //             }
            //             Err(e) => error!("{:?}", e),
            //         }
            //     }
            //     need_rank = false;
            // }
        }
        v
    }

    ///使用技能后触发
    fn after_use_skill_trigger(
        &mut self,
        user_id: u32,
        skill_id: u32,
        is_item: bool,
        au: &mut ActionUnitPt,
    ) {
        let cter = self.get_battle_cter_mut(Some(user_id), true);
        if let Err(e) = cter {
            error!("{:?}", e);
            return;
        }
        let cter = cter.unwrap();
        //战斗角色身上的技能
        let mut skill_s;
        let skill;
        if is_item {
            let res = TEMPLATES.get_skill_temp_mgr_ref().get_temp(&skill_id);
            if let Err(e) = res {
                error!("{:?}", e);
                return;
            }
            let skill_temp = res.unwrap();
            skill_s = Some(Skill::from(skill_temp));
            skill = skill_s.as_mut().unwrap();
        } else {
            skill = cter.skills.get_mut(&skill_id).unwrap();
        }
        //替换技能,水炮
        if skill.id == WATER_TURRET {
            let skill_temp = TEMPLATES
                .get_skill_temp_mgr_ref()
                .get_temp(&skill.skill_temp.par2);
            cter.skills.remove(&skill_id);
            if let Err(e) = skill_temp {
                error!("{:?}", e);
                return;
            }
            let st = skill_temp.unwrap();

            let mut target_pt = TargetPt::new();
            //封装角色位置
            target_pt
                .target_value
                .push(cter.get_map_cell_index() as u32);
            //封装丢失技能
            target_pt.lost_buffs.push(skill_id);
            //封装增加的技能
            let mut ep = EffectPt::new();
            ep.effect_type = AddSkill.into_u32();
            ep.effect_value = st.par2;
            target_pt.effects.push(ep);
            //将新技能封装到内存
            let skill = Skill::from(st);
            cter.skills.insert(skill.id, skill);
            //将target封装到proto
            au.targets.push(target_pt);
        }

        //添加技能限制条件
        let skill_temp = TEMPLATES
            .get_skill_temp_mgr_ref()
            .get_temp(&skill_id)
            .unwrap();
        if skill_temp.skill_judge == LIMIT_TURN_TIMES as u16 {
            cter.flow_data.turn_limit_skills.push(skill_id);
        } else if skill_temp.skill_judge == LIMIT_ROUND_TIMES as u16 {
            cter.flow_data.turn_limit_skills.push(skill_id);
        }
    }

    fn before_use_skill_trigger(&mut self, user_id: u32) -> anyhow::Result<()> {
        let cter = self.get_battle_cter_mut(Some(user_id), true);
        if let Err(e) = cter {
            anyhow::bail!("{:?}", e)
        }
        let cter = cter.unwrap();
        for buff_id in cter.battle_buffs.buffs.keys() {
            if LOCK_SKILLS.contains(buff_id) {
                anyhow::bail!("this cter can not use skill!cter's buff:{}", buff_id)
            }
        }
        Ok(())
    }

    ///受到普通攻击触发的buff
    fn attacked_buffs_trigger(&mut self, user_id: u32, target_pt: &mut TargetPt) {
        let battle_data = self as *mut BattleData;
        let cter = self.get_battle_cter_mut(Some(user_id), true).unwrap();
        let max_energy = cter.base_attr.max_energy;
        for buff in cter.battle_buffs.buffs.clone().values() {
            let buff_id = buff.id;
            //被攻击打断技能
            if CHANGE_SKILL.contains(&buff_id) {
                unsafe {
                    battle_data
                        .as_mut()
                        .unwrap()
                        .consume_buff(buff_id, Some(user_id), None, false);
                }
                target_pt.lost_buffs.push(buff_id);
            }

            //被攻击增加能量
            if ATTACKED_ADD_ENERGY.contains(&buff_id) && max_energy > 0 {
                let mut tep = TriggerEffectPt::new();
                tep.set_field_type(EffectType::AddEnergy.into_u32());
                tep.set_buff_id(buff_id);
                tep.set_value(buff.buff_temp.par1);
                cter.add_energy(buff.buff_temp.par1 as i8);
                target_pt.passiveEffect.push(tep);
            }
        }
    }

    fn before_map_refresh_buff_trigger(&mut self) {
        //如果存活玩家>=2并且地图未配对的数量<=2则刷新地图
        for map_cell in self.tile_map.map_cells.iter() {
            let buff = map_cell.buffs.get(&LOCKED);
            if buff.is_none() {
                continue;
            }
            let buff = buff.unwrap();
            let from_user = buff.from_user;
            if from_user.is_none() {
                continue;
            }
            let from_user = from_user.unwrap();
            let from_skill = buff.from_skill.unwrap();
            let cter = self.battle_cter.get_mut(&from_user);
            if cter.is_none() {
                continue;
            }
            let cter = cter.unwrap();
            let skill = cter.skills.get_mut(&from_skill);
            if skill.is_none() {
                continue;
            }
            let skill = skill.unwrap();
            skill.is_active = false;
        }
    }

    ///buff失效时候触发
    fn buff_lost_trigger(&mut self, user_id: u32, buff_id: u32) {
        let cter = self.get_battle_cter_mut(Some(user_id), true);
        if let Err(e) = cter {
            error!("{:?}", e);
            return;
        }
        let cter = cter.unwrap();
        //如果是变身buff,那就变回来
        if TRANSFORM_BUFF.contains(&buff_id) {
            cter.transform_back();
        }
    }
}
