use std::time::Duration;

pub const WALK_BLOCKS_PER_SECOND: f64 = 4.3;
pub const SPRINT_BLOCKS_PER_SECOND: f64 = 5.6;

#[derive(Clone, Copy)]
pub enum BotModel {
    Idle,
    Sentry,
    Wanderer,
    Runner,
    Voyager,
}

pub struct ContinuousMovement {
    pub tick: Duration,
    pub blocks_per_tick: f64,
    pub keep_heading: bool,
}

pub struct MovementProfile {
    pub disperse: bool,
    pub continuous: Option<ContinuousMovement>,
}

impl BotModel {
    pub fn parse(value: &str) -> BotModel {
        match value {
            "idle" => BotModel::Idle,
            "wanderer" => BotModel::Wanderer,
            "runner" => BotModel::Runner,
            "voyager" => BotModel::Voyager,
            _ => BotModel::Sentry,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            BotModel::Idle => "idle",
            BotModel::Sentry => "sentry",
            BotModel::Wanderer => "wanderer",
            BotModel::Runner => "runner",
            BotModel::Voyager => "voyager",
        }
    }

    pub fn profile(&self) -> MovementProfile {
        match self {
            BotModel::Idle => MovementProfile {
                disperse: false,
                continuous: None,
            },
            BotModel::Sentry => MovementProfile {
                disperse: true,
                continuous: None,
            },
            BotModel::Wanderer => MovementProfile {
                disperse: true,
                continuous: Some(ContinuousMovement {
                    tick: Duration::from_secs(1),
                    blocks_per_tick: 0.6,
                    keep_heading: false,
                }),
            },
            BotModel::Runner => MovementProfile {
                disperse: true,
                continuous: Some(ContinuousMovement {
                    tick: Duration::from_millis(100),
                    blocks_per_tick: WALK_BLOCKS_PER_SECOND / 10.0,
                    keep_heading: true,
                }),
            },
            BotModel::Voyager => MovementProfile {
                disperse: true,
                continuous: Some(ContinuousMovement {
                    tick: Duration::from_millis(50),
                    blocks_per_tick: SPRINT_BLOCKS_PER_SECOND / 20.0,
                    keep_heading: true,
                }),
            },
        }
    }
}
