pub mod v1;

pub mod prelude {
    pub use crate::v1::aws::{
        api::{integration::*, resource::*, restapi::*},
        ec2::{subnet::*, vpc::*, *},
        iam::{
            attach_group_policy::*, attach_role_policy::*, attach_user_policy::*, group::*,
            policy::*, role::*, user::*, *,
        },
        lambda::{function::*, *},
        s3::{bucket::*, object::*, *},
        scheduler::{schedule::*, *},
        sfn::{schema::*, state_machine::*, *},
        *,
    };
    pub use crate::v1::cloud::*;
    pub use crate::v1::resource::{ResourceState::*, *};
    pub use crate::v1::solutions::lambda::*;
}
