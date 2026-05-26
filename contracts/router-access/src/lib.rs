#![no_std]

//! # router-access
//!
//! Role-based access control for the stellar-router suite.
//! Supports arbitrary roles, multi-admin, per-address whitelisting,
//! and a role hierarchy where parent roles implicitly include child roles.
//!
//! ## Role Hierarchy
//!
//! Roles can be arranged in a parent → child relationship. Granting a parent
//! role to an address implicitly grants all of its child roles (transitively).
//! For example, if `admin` is the parent of `editor`, and `editor` is the
//! parent of `viewer`, then an address with `admin` also has `editor` and
//! `viewer` without needing explicit grants.
//!
//! The hierarchy is stored as a directed acyclic graph (DAG). Cycles are
//! prevented by `set_role_parent` — a role cannot be set as its own ancestor.
//!
//! ## Storage model
//!
//! - `HasRole(role, address)` — explicit direct grant
//! - `RoleParent(role)` — the single parent role of `role` (if any)
//! - `RoleAdmin(role)` — address allowed to grant/revoke `role`

use soroban_sdk::{contract, contractimpl, contracttype, contracterror, Address, Env, String, Symbol};

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    SuperAdmin,
    HasRole(String, Address),  // (role, address) -> bool  (direct grant only)
    RoleAdmin(String),         // role -> Address who manages it
    Blacklisted(Address),
    RoleParent(String),        // role -> parent role name (hierarchy edge)
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AccessError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    AlreadyHasRole = 4,
    RoleNotFound = 5,
    Blacklisted = 6,
    CannotBlacklistAdmin = 7,
    HierarchyCycle = 8,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct RouterAccess;

// Maximum depth to walk when resolving inherited roles. Prevents infinite
// loops in the unlikely event of a storage inconsistency.
const MAX_HIERARCHY_DEPTH: u32 = 16;

#[contractimpl]
impl RouterAccess {
    /// Initialize with a super-admin.
    ///
    /// # Errors
    /// * [`AccessError::AlreadyInitialized`] — called more than once.
    pub fn initialize(env: Env, super_admin: Address) -> Result<(), AccessError> {
        if env.storage().instance().has(&DataKey::SuperAdmin) {
            return Err(AccessError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::SuperAdmin, &super_admin);
        Ok(())
    }

    /// Grant a role to an address. Caller must be super-admin or role admin.
    ///
    /// Only the direct role is stored. Inherited roles are resolved at
    /// check time via [`Self::has_role`].
    ///
    /// # Errors
    /// * [`AccessError::Unauthorized`] — caller is not super-admin or role admin.
    /// * [`AccessError::AlreadyHasRole`] — target already holds the role directly.
    /// * [`AccessError::Blacklisted`] — target is blacklisted.
    pub fn grant_role(
        env: Env,
        caller: Address,
        role: String,
        target: Address,
    ) -> Result<(), AccessError> {
        caller.require_auth();
        Self::require_role_manager(&env, &caller, &role)?;

        if Self::has_direct_role(&env, &role, &target) {
            return Err(AccessError::AlreadyHasRole);
        }
        if Self::is_blacklisted_internal(&env, &target) {
            return Err(AccessError::Blacklisted);
        }

        env.storage()
            .instance()
            .set(&DataKey::HasRole(role.clone(), target.clone()), &true);

        env.events().publish(
            (Symbol::new(&env, "role_granted"),),
            (role, target),
        );
        Ok(())
    }

    /// Revoke a direct role grant from an address.
    ///
    /// Only removes the direct grant. If the address inherits the role via
    /// the hierarchy it will still pass `has_role` checks.
    ///
    /// # Errors
    /// * [`AccessError::Unauthorized`] — caller is not super-admin or role admin.
    /// * [`AccessError::RoleNotFound`] — target does not hold the role directly.
    pub fn revoke_role(
        env: Env,
        caller: Address,
        role: String,
        target: Address,
    ) -> Result<(), AccessError> {
        caller.require_auth();
        Self::require_role_manager(&env, &caller, &role)?;

        let key = DataKey::HasRole(role.clone(), target.clone());
        if !env.storage().instance().has(&key) {
            return Err(AccessError::RoleNotFound);
        }

        env.storage().instance().remove(&key);

        env.events().publish(
            (Symbol::new(&env, "role_revoked"),),
            (role, target),
        );
        Ok(())
    }

    /// Check if an address has a role — either directly or via the hierarchy.
    ///
    /// Walks the role's ancestor chain. Returns `true` if the address holds
    /// any role in the chain from `role` up to the root.
    pub fn has_role(env: Env, role: String, target: Address) -> bool {
        if Self::is_blacklisted_internal(&env, &target) {
            return false;
        }
        Self::has_role_internal(&env, &role, &target)
    }

    /// Set the parent role for a role (defines the hierarchy edge).
    ///
    /// After this call, any address that holds `parent_role` (directly or
    /// via inheritance) will also pass `has_role` checks for `role`.
    ///
    /// Only the super-admin can modify the hierarchy.
    ///
    /// # Errors
    /// * [`AccessError::Unauthorized`] — caller is not the super-admin.
    /// * [`AccessError::HierarchyCycle`] — setting this parent would create a cycle.
    pub fn set_role_parent(
        env: Env,
        caller: Address,
        role: String,
        parent_role: String,
    ) -> Result<(), AccessError> {
        caller.require_auth();
        Self::require_super_admin(&env, &caller)?;

        // Prevent cycles: parent_role must not be a descendant of role.
        // Equivalently, role must not appear in parent_role's ancestor chain.
        if Self::is_ancestor(&env, &parent_role, &role) {
            return Err(AccessError::HierarchyCycle);
        }

        env.storage()
            .instance()
            .set(&DataKey::RoleParent(role.clone()), &parent_role);

        env.events().publish(
            (Symbol::new(&env, "role_parent_set"),),
            (role, parent_role),
        );
        Ok(())
    }

    /// Remove the parent relationship for a role.
    ///
    /// After this call, `role` becomes a root role with no parent.
    /// Only the super-admin can modify the hierarchy.
    ///
    /// # Errors
    /// * [`AccessError::Unauthorized`] — caller is not the super-admin.
    pub fn remove_role_parent(
        env: Env,
        caller: Address,
        role: String,
    ) -> Result<(), AccessError> {
        caller.require_auth();
        Self::require_super_admin(&env, &caller)?;
        env.storage().instance().remove(&DataKey::RoleParent(role.clone()));
        env.events().publish(
            (Symbol::new(&env, "role_parent_removed"),),
            role,
        );
        Ok(())
    }

    /// Get the direct parent role of a role, if one is set.
    pub fn get_role_parent(env: Env, role: String) -> Option<String> {
        env.storage()
            .instance()
            .get(&DataKey::RoleParent(role))
    }

    /// Set the admin for a specific role (who can grant/revoke it).
    ///
    /// # Errors
    /// * [`AccessError::Unauthorized`] — caller is not the super-admin.
    pub fn set_role_admin(
        env: Env,
        caller: Address,
        role: String,
        admin: Address,
    ) -> Result<(), AccessError> {
        caller.require_auth();
        Self::require_super_admin(&env, &caller)?;
        env.storage().instance().set(&DataKey::RoleAdmin(role), &admin);
        Ok(())
    }

    /// Blacklist an address — prevents it from being granted any role.
    ///
    /// # Errors
    /// * [`AccessError::Unauthorized`] — caller is not the super-admin.
    /// * [`AccessError::CannotBlacklistAdmin`] — target is the super-admin.
    pub fn blacklist(env: Env, caller: Address, target: Address) -> Result<(), AccessError> {
        caller.require_auth();
        Self::require_super_admin(&env, &caller)?;

        let super_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::SuperAdmin)
            .ok_or(AccessError::NotInitialized)?;
        if target == super_admin {
            return Err(AccessError::CannotBlacklistAdmin);
        }

        env.storage().instance().set(&DataKey::Blacklisted(target), &true);
        Ok(())
    }

    /// Remove an address from the blacklist.
    ///
    /// # Errors
    /// * [`AccessError::Unauthorized`] — caller is not the super-admin.
    pub fn unblacklist(env: Env, caller: Address, target: Address) -> Result<(), AccessError> {
        caller.require_auth();
        Self::require_super_admin(&env, &caller)?;
        env.storage().instance().set(&DataKey::Blacklisted(target), &false);
        Ok(())
    }

    /// Check if an address is blacklisted.
    pub fn is_blacklisted(env: Env, target: Address) -> bool {
        Self::is_blacklisted_internal(&env, &target)
    }

    /// Transfer super-admin to a new address.
    ///
    /// # Errors
    /// * [`AccessError::Unauthorized`] — caller is not the current super-admin.
    pub fn transfer_super_admin(
        env: Env,
        current: Address,
        new_admin: Address,
    ) -> Result<(), AccessError> {
        current.require_auth();
        Self::require_super_admin(&env, &current)?;
        env.storage().instance().set(&DataKey::SuperAdmin, &new_admin);
        env.events().publish(
            (Symbol::new(&env, "admin_transferred"),),
            (current, new_admin),
        );
        Ok(())
    }

    /// Get current super-admin.
    ///
    /// # Errors
    /// * [`AccessError::NotInitialized`] — contract not initialized.
    pub fn super_admin(env: Env) -> Result<Address, AccessError> {
        env.storage()
            .instance()
            .get(&DataKey::SuperAdmin)
            .ok_or(AccessError::NotInitialized)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn require_super_admin(env: &Env, caller: &Address) -> Result<(), AccessError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::SuperAdmin)
            .ok_or(AccessError::NotInitialized)?;
        if &admin != caller {
            return Err(AccessError::Unauthorized);
        }
        Ok(())
    }

    fn require_role_manager(env: &Env, caller: &Address, role: &String) -> Result<(), AccessError> {
        if let Some(admin) = env.storage().instance().get::<DataKey, Address>(&DataKey::SuperAdmin) {
            if &admin == caller {
                return Ok(());
            }
        }
        if let Some(role_admin) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::RoleAdmin(role.clone()))
        {
            if &role_admin == caller {
                return Ok(());
            }
        }
        Err(AccessError::Unauthorized)
    }

    /// Returns true if `target` holds `role` directly (no hierarchy walk).
    fn has_direct_role(env: &Env, role: &String, target: &Address) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::HasRole(role.clone(), target.clone()))
            .unwrap_or(false)
    }

    /// Returns true if `target` holds `role` directly OR via the hierarchy.
    ///
    /// Walks up the ancestor chain of `role`. At each level, checks whether
    /// `target` has a direct grant for that ancestor. Stops at depth
    /// `MAX_HIERARCHY_DEPTH` to guard against storage inconsistencies.
    fn has_role_internal(env: &Env, role: &String, target: &Address) -> bool {
        let mut current = role.clone();
        let mut depth = 0u32;

        loop {
            // Direct grant at this level?
            if Self::has_direct_role(env, &current, target) {
                return true;
            }

            // Walk up to parent
            match env.storage().instance().get::<DataKey, String>(&DataKey::RoleParent(current)) {
                Some(parent) => {
                    depth += 1;
                    if depth >= MAX_HIERARCHY_DEPTH {
                        return false;
                    }
                    current = parent;
                }
                None => return false,
            }
        }
    }

    /// Returns true if `ancestor` is an ancestor of `role` in the hierarchy.
    /// Used by `set_role_parent` to detect cycles.
    fn is_ancestor(env: &Env, role: &String, ancestor: &String) -> bool {
        let mut current = role.clone();
        let mut depth = 0u32;

        loop {
            if &current == ancestor {
                return true;
            }
            match env.storage().instance().get::<DataKey, String>(&DataKey::RoleParent(current)) {
                Some(parent) => {
                    depth += 1;
                    if depth >= MAX_HIERARCHY_DEPTH {
                        return false;
                    }
                    current = parent;
                }
                None => return false,
            }
        }
    }

    fn is_blacklisted_internal(env: &Env, target: &Address) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Blacklisted(target.clone()))
            .unwrap_or(false)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, String};

    fn setup() -> (Env, Address, RouterAccessClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RouterAccess);
        let client = RouterAccessClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, admin, client)
    }

    // ── Existing tests ────────────────────────────────────────────────────────

    #[test]
    fn test_grant_and_check_role() {
        let (env, admin, client) = setup();
        let role = String::from_str(&env, "operator");
        let user = Address::generate(&env);
        client.grant_role(&admin, &role, &user);
        assert!(client.has_role(&role, &user));
    }

    #[test]
    fn test_revoke_role() {
        let (env, admin, client) = setup();
        let role = String::from_str(&env, "operator");
        let user = Address::generate(&env);
        client.grant_role(&admin, &role, &user);
        client.revoke_role(&admin, &role, &user);
        assert!(!client.has_role(&role, &user));
    }

    #[test]
    fn test_double_grant_fails() {
        let (env, admin, client) = setup();
        let role = String::from_str(&env, "operator");
        let user = Address::generate(&env);
        client.grant_role(&admin, &role, &user);
        let result = client.try_grant_role(&admin, &role, &user);
        assert_eq!(result, Err(Ok(AccessError::AlreadyHasRole)));
    }

    #[test]
    fn test_blacklist_prevents_grant() {
        let (env, admin, client) = setup();
        let role = String::from_str(&env, "operator");
        let user = Address::generate(&env);
        client.blacklist(&admin, &user);
        let result = client.try_grant_role(&admin, &role, &user);
        assert_eq!(result, Err(Ok(AccessError::Blacklisted)));
    }

    #[test]
    fn test_cannot_blacklist_admin() {
        let (env, admin, client) = setup();
        let result = client.try_blacklist(&admin, &admin);
        assert_eq!(result, Err(Ok(AccessError::CannotBlacklistAdmin)));
    }

    #[test]
    fn test_role_admin_can_grant() {
        let (env, admin, client) = setup();
        let role = String::from_str(&env, "operator");
        let role_admin = Address::generate(&env);
        let user = Address::generate(&env);
        client.set_role_admin(&admin, &role, &role_admin);
        client.grant_role(&role_admin, &role, &user);
        assert!(client.has_role(&role, &user));
    }

    #[test]
    fn test_unauthorized_grant_fails() {
        let (env, _admin, client) = setup();
        let role = String::from_str(&env, "operator");
        let attacker = Address::generate(&env);
        let user = Address::generate(&env);
        let result = client.try_grant_role(&attacker, &role, &user);
        assert_eq!(result, Err(Ok(AccessError::Unauthorized)));
    }

    #[test]
    fn test_transfer_super_admin() {
        let (env, admin, client) = setup();
        let new_admin = Address::generate(&env);
        client.transfer_super_admin(&admin, &new_admin);
        assert_eq!(client.super_admin(), new_admin);
    }

    // ── Role hierarchy tests ──────────────────────────────────────────────────

    #[test]
    fn test_parent_role_grants_child_access() {
        let (env, admin, client) = setup();
        let viewer = String::from_str(&env, "viewer");
        let editor = String::from_str(&env, "editor");
        let user = Address::generate(&env);

        // editor is parent of viewer: editor → viewer
        client.set_role_parent(&admin, &viewer, &editor);

        // Grant editor to user
        client.grant_role(&admin, &editor, &user);

        // User should have both editor (direct) and viewer (inherited)
        assert!(client.has_role(&editor, &user));
        assert!(client.has_role(&viewer, &user));
    }

    #[test]
    fn test_transitive_hierarchy() {
        let (env, admin, client) = setup();
        let viewer = String::from_str(&env, "viewer");
        let editor = String::from_str(&env, "editor");
        let admin_role = String::from_str(&env, "admin");
        let user = Address::generate(&env);

        // admin → editor → viewer
        client.set_role_parent(&admin, &editor, &admin_role);
        client.set_role_parent(&admin, &viewer, &editor);

        // Grant admin to user
        client.grant_role(&admin, &admin_role, &user);

        // User should have all three roles
        assert!(client.has_role(&admin_role, &user));
        assert!(client.has_role(&editor, &user));
        assert!(client.has_role(&viewer, &user));
    }

    #[test]
    fn test_no_inheritance_without_parent() {
        let (env, admin, client) = setup();
        let viewer = String::from_str(&env, "viewer");
        let editor = String::from_str(&env, "editor");
        let user = Address::generate(&env);

        // No parent set — roles are independent
        client.grant_role(&admin, &editor, &user);
        assert!(client.has_role(&editor, &user));
        assert!(!client.has_role(&viewer, &user));
    }

    #[test]
    fn test_set_role_parent_cycle_fails() {
        let (env, admin, client) = setup();
        let a = String::from_str(&env, "a");
        let b = String::from_str(&env, "b");

        // a → b
        client.set_role_parent(&admin, &b, &a);

        // b → a would create a cycle
        let result = client.try_set_role_parent(&admin, &a, &b);
        assert_eq!(result, Err(Ok(AccessError::HierarchyCycle)));
    }

    #[test]
    fn test_self_cycle_fails() {
        let (env, admin, client) = setup();
        let role = String::from_str(&env, "admin");
        let result = client.try_set_role_parent(&admin, &role, &role);
        assert_eq!(result, Err(Ok(AccessError::HierarchyCycle)));
    }

    #[test]
    fn test_remove_role_parent_breaks_inheritance() {
        let (env, admin, client) = setup();
        let viewer = String::from_str(&env, "viewer");
        let editor = String::from_str(&env, "editor");
        let user = Address::generate(&env);

        client.set_role_parent(&admin, &viewer, &editor);
        client.grant_role(&admin, &editor, &user);
        assert!(client.has_role(&viewer, &user));

        // Remove the parent link
        client.remove_role_parent(&admin, &viewer);
        assert!(!client.has_role(&viewer, &user));
        // Direct role still works
        assert!(client.has_role(&editor, &user));
    }

    #[test]
    fn test_get_role_parent() {
        let (env, admin, client) = setup();
        let viewer = String::from_str(&env, "viewer");
        let editor = String::from_str(&env, "editor");

        assert_eq!(client.get_role_parent(&viewer), None);
        client.set_role_parent(&admin, &viewer, &editor);
        assert_eq!(client.get_role_parent(&viewer), Some(editor));
    }

    #[test]
    fn test_blacklisted_user_fails_has_role_even_with_hierarchy() {
        let (env, admin, client) = setup();
        let viewer = String::from_str(&env, "viewer");
        let editor = String::from_str(&env, "editor");
        let user = Address::generate(&env);

        client.set_role_parent(&admin, &viewer, &editor);
        client.grant_role(&admin, &editor, &user);
        assert!(client.has_role(&viewer, &user));

        client.blacklist(&admin, &user);
        assert!(!client.has_role(&viewer, &user));
        assert!(!client.has_role(&editor, &user));
    }

    #[test]
    fn test_unauthorized_set_role_parent_fails() {
        let (env, _admin, client) = setup();
        let attacker = Address::generate(&env);
        let a = String::from_str(&env, "a");
        let b = String::from_str(&env, "b");
        let result = client.try_set_role_parent(&attacker, &a, &b);
        assert_eq!(result, Err(Ok(AccessError::Unauthorized)));
    }
}
