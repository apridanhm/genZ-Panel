use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::root,
        crate::handlers::health_check,
        crate::handlers::register,
        crate::handlers::login,
        crate::handlers::get_current_user,
        crate::handlers::create_domain,
        crate::handlers::list_domains,
        crate::handlers::get_domain,
        crate::handlers::update_domain,
        crate::handlers::delete_domain,
    ),
    components(
        schemas(
            crate::models::RegisterRequest,
            crate::models::LoginRequest,
            crate::models::AuthResponse,
            crate::models::UserResponse,
            crate::models::CreateDomainRequest,
            crate::models::UpdateDomainRequest,
            crate::models::DomainResponse,
            crate::handlers::RootResponse,
            crate::handlers::HealthResponse,
            crate::handlers::UserMeResponse,
            crate::handlers::DomainListResponse,
        )
    ),
    tags(
        (name = "GenZ Panel API", description = "Hosting Panel API endpoints")
    ),
    modifiers(&SecurityAddon),
    servers(
        (url = "http://localhost:8000", description = "Local development")
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            )
        }
    }
}
