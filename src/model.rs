use std::time::Duration;

pub const WALK_BLOCKS_PER_SECOND: f64 = 4.3;
pub const SPRINT_BLOCKS_PER_SECOND: f64 = 5.6;

#[derive(Clone, Copy)]
pub enum BotModel {
    Idle,
    Sentry,
    Spread,
    Wanderer,
    Movement,
    Runner,
    Voyager,
}

pub struct ContinuousMovement {
    pub tick: Duration,
    pub blocks_per_tick: f64,
    pub keep_heading: bool,
    pub oscillate_in_place: bool,
}

pub struct MovementProfile {
    pub disperse: bool,
    pub disperse_scale: f64,
    pub continuous: Option<ContinuousMovement>,
}

impl BotModel {
    pub fn parse(value: &str) -> BotModel {
        match value {
            "idle" => BotModel::Idle,
            "spread" => BotModel::Spread,
            "wanderer" => BotModel::Wanderer,
            "movement" => BotModel::Movement,
            "runner" => BotModel::Runner,
            "voyager" => BotModel::Voyager,
            _ => BotModel::Sentry,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            BotModel::Idle => "idle",
            BotModel::Sentry => "sentry",
            BotModel::Spread => "spread",
            BotModel::Wanderer => "wanderer",
            BotModel::Movement => "movement",
            BotModel::Runner => "runner",
            BotModel::Voyager => "voyager",
        }
    }

    pub fn profile(&self) -> MovementProfile {
        match self {
            BotModel::Idle => MovementProfile {
                disperse: false,
                disperse_scale: 1.0,
                continuous: None,
            },
            BotModel::Sentry => MovementProfile {
                disperse: true,
                disperse_scale: 1.0,
                continuous: None,
            },
            BotModel::Spread => MovementProfile {
                disperse: true,
                disperse_scale: 4.0,
                continuous: None,
            },
            BotModel::Wanderer => MovementProfile {
                disperse: true,
                disperse_scale: 1.0,
                continuous: Some(ContinuousMovement {
                    tick: Duration::from_secs(1),
                    blocks_per_tick: 0.6,
                    keep_heading: false,
                    oscillate_in_place: false,
                }),
            },
            BotModel::Movement => MovementProfile {
                disperse: false,
                disperse_scale: 1.0,
                continuous: Some(ContinuousMovement {
                    tick: Duration::from_millis(50),
                    blocks_per_tick: 0.3,
                    keep_heading: true,
                    oscillate_in_place: true,
                }),
            },
            BotModel::Runner => MovementProfile {
                disperse: true,
                disperse_scale: 1.0,
                continuous: Some(ContinuousMovement {
                    tick: Duration::from_millis(100),
                    blocks_per_tick: WALK_BLOCKS_PER_SECOND / 10.0,
                    keep_heading: true,
                    oscillate_in_place: false,
                }),
            },
            BotModel::Voyager => MovementProfile {
                disperse: true,
                disperse_scale: 1.0,
                continuous: Some(ContinuousMovement {
                    tick: Duration::from_millis(50),
                    blocks_per_tick: SPRINT_BLOCKS_PER_SECOND / 20.0,
                    keep_heading: true,
                    oscillate_in_place: false,
                }),
            },
        }
    }
}
