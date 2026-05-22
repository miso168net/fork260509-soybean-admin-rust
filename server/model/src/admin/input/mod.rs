pub use sys_access_key::{AccessKeyPageRequest, CreateAccessKeyInput};
pub use sys_authentication::{AuthErrorQuery, LoginInput, RefreshTokenInput, SendCaptchaInput, VerifyCaptchaInput};
pub use sys_authorization::{AssignPermissionDto, AssignRouteDto, AssignUserDto};
pub use sys_domain::{CreateDomainInput, DomainPageRequest, UpdateDomainInput};
pub use sys_endpoint::EndpointPageRequest;
pub use sys_login_log::LoginLogPageRequest;
pub use sys_menu::{
    BatchDeleteMenuInput, CreateMenuInput, DeleteMenuByBodyInput, IsRouteExistInput, MenuInput,
    SystemManageAddMenuInput, SystemManageUpdateMenuInput, UpdateMenuInput,
};
pub use sys_operation_log::OperationLogPageRequest;
pub use sys_organization::OrganizationPageRequest;
pub use sys_role::{
    BatchDeleteRoleInput, CreateRoleInput, DeleteRoleByBodyInput, RolePageRequest,
    SystemManageAddRoleInput, SystemManageUpdateRoleInput, UpdateRoleInput,
};
pub use sys_user::{
    BatchDeleteUserInput, CreateUserInput, DeleteUserByBodyInput, SystemManageAddUserInput,
    SystemManageUpdateUserInput, UpdateUserInput, UserPageRequest,
};

mod sys_access_key;
mod sys_authentication;
mod sys_authorization;
mod sys_domain;
mod sys_endpoint;
mod sys_login_log;
mod sys_menu;
mod sys_operation_log;
mod sys_organization;
mod sys_role;
mod sys_user;
