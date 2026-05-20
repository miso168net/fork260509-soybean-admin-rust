pub use sys_authentication::{AuthOutput, UserInfoOutput, UserRoute};
pub use sys_domain::DomainOutput;
pub use sys_endpoint::EndpointTree;
pub use sys_menu::{MenuRoute, MenuTree, RouteMeta};
pub use sys_system_manage::{
    SystemManageAllRoleOutput, SystemManageMenuOutput, SystemManageMenuTreeNodeOutput,
    SystemManageRoleOutput, SystemManageUserOutput,
};
pub use sys_user::{UserWithDomainAndOrgOutput, UserWithoutPassword};

mod sys_authentication;
mod sys_domain;
mod sys_endpoint;
mod sys_menu;
pub mod sys_system_manage;
mod sys_user;
