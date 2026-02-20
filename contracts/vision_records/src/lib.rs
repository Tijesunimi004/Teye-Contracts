#![no_std]

pub mod events;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

/// Storage keys for the contract
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

/// User roles in the vision care system
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Patient,
    Optometrist,
    Ophthalmologist,
    Admin,
}

/// Access levels for record sharing
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessLevel {
    None,
    Read,
    Write,
    Full,
}

/// Vision record types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordType {
    Examination,
    Prescription,
    Diagnosis,
    Treatment,
    Surgery,
    LabResult,
}

/// User information structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct User {
    pub address: Address,
    pub role: Role,
    pub name: String,
    pub registered_at: u64,
    pub is_active: bool,
}

/// Vision record structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct VisionRecord {
    pub id: u64,
    pub patient: Address,
    pub provider: Address,
    pub record_type: RecordType,
    pub data_hash: String,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Access grant structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct AccessGrant {
    pub patient: Address,
    pub grantee: Address,
    pub level: AccessLevel,
    pub granted_at: u64,
    pub expires_at: u64,
}

/// Contract errors
/// Contract errors
#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    UserNotFound = 4,
    RecordNotFound = 5,
    InvalidInput = 6,
    AccessDenied = 7,
    Paused = 8,
}

#[contract]
pub struct VisionRecordsContract;

#[contractimpl]
impl VisionRecordsContract {
    /// Initialize the contract with an admin address
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::AlreadyInitialized);
        }

        // admin.require_auth();

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);

        events::publish_initialized(&env, admin);

        Ok(())
    }

    /// Get the admin address
    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)
    }

    /// Check if the contract is initialized
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&INITIALIZED)
    }

    /// Register a new user
    pub fn register_user(
        env: Env,
        user: Address,
        role: Role,
        name: String,
    ) -> Result<(), ContractError> {
        user.require_auth();

        let user_data = User {
            address: user.clone(),
            role: role.clone(),
            name: name.clone(),
            registered_at: env.ledger().timestamp(),
            is_active: true,
        };

        let key = (symbol_short!("USER"), user.clone());
        env.storage().persistent().set(&key, &user_data);

        events::publish_user_registered(&env, user, role, name);

        Ok(())
    }

    /// Get user information
    pub fn get_user(env: Env, user: Address) -> Result<User, ContractError> {
        let key = (symbol_short!("USER"), user);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::UserNotFound)
    }

    /// Add a vision record
    pub fn add_record(
        env: Env,
        patient: Address,
        provider: Address,
        record_type: RecordType,
        data_hash: String,
    ) -> Result<u64, ContractError> {
        provider.require_auth();

        // Generate record ID
        let counter_key = symbol_short!("REC_CTR");
        let record_id: u64 = env.storage().instance().get(&counter_key).unwrap_or(0) + 1;
        env.storage().instance().set(&counter_key, &record_id);

        let record = VisionRecord {
            id: record_id,
            patient: patient.clone(),
            provider: provider.clone(),
            record_type: record_type.clone(),
            data_hash,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
        };

        let key = (symbol_short!("RECORD"), record_id);
        env.storage().persistent().set(&key, &record);

        // Add to patient's record list
        let patient_key = (symbol_short!("PAT_REC"), patient.clone());
        let mut patient_records: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));
        patient_records.push_back(record_id);
        env.storage()
            .persistent()
            .set(&patient_key, &patient_records);

        events::publish_record_added(&env, record_id, patient, provider, record_type);

        Ok(record_id)
    }

    /// Get a vision record by ID
    pub fn get_record(env: Env, record_id: u64) -> Result<VisionRecord, ContractError> {
        let key = (symbol_short!("RECORD"), record_id);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::RecordNotFound)
    }

    /// Get all records for a patient
    pub fn get_patient_records(env: Env, patient: Address) -> Vec<u64> {
        let key = (symbol_short!("PAT_REC"), patient);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env))
    }

    /// Grant access to a user
    pub fn grant_access(
        env: Env,
        patient: Address,
        grantee: Address,
        level: AccessLevel,
        duration_seconds: u64,
    ) -> Result<(), ContractError> {
        patient.require_auth();

        let expires_at = env.ledger().timestamp() + duration_seconds;
        let grant = AccessGrant {
            patient: patient.clone(),
            grantee: grantee.clone(),
            level: level.clone(),
            granted_at: env.ledger().timestamp(),
            expires_at,
        };

        let key = (symbol_short!("ACCESS"), patient.clone(), grantee.clone());
        env.storage().persistent().set(&key, &grant);

        events::publish_access_granted(&env, patient, grantee, level, duration_seconds, expires_at);

        Ok(())
    }

    /// Check access level
    pub fn check_access(env: Env, patient: Address, grantee: Address) -> AccessLevel {
        let key = (symbol_short!("ACCESS"), patient, grantee);

        if let Some(grant) = env.storage().persistent().get::<_, AccessGrant>(&key) {
            if grant.expires_at > env.ledger().timestamp() {
                return grant.level;
            }
        }

        AccessLevel::None
    }

    /// Revoke access
    pub fn revoke_access(
        env: Env,
        patient: Address,
        grantee: Address,
    ) -> Result<(), ContractError> {
        patient.require_auth();

        let key = (symbol_short!("ACCESS"), patient.clone(), grantee.clone());
        env.storage().persistent().remove(&key);

        events::publish_access_revoked(&env, patient, grantee);

        Ok(())
    }

    /// Get the total number of records
    pub fn get_record_count(env: Env) -> u64 {
        let counter_key = symbol_short!("REC_CTR");
        env.storage().instance().get(&counter_key).unwrap_or(0)
    }

    /// Contract version
    pub fn version() -> u32 {
        1
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events};
    use soroban_sdk::{Env, IntoVal, TryIntoVal};

    #[test]
    fn test_initialize() {
        let env = Env::default();
        // env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();

        assert!(client.is_initialized());
        assert_eq!(client.get_admin(), admin);
        let our_events: soroban_sdk::Vec<(
            soroban_sdk::Address,
            soroban_sdk::Vec<soroban_sdk::Val>,
            soroban_sdk::Val,
        )> = events;

        assert!(!our_events.is_empty());
        let event = our_events.get(our_events.len() - 1).unwrap();
        assert_eq!(event.1, (symbol_short!("INIT"),).into_val(&env));
        let payload: events::InitializedEvent = event.2.try_into_val(&env).unwrap();
        assert_eq!(payload.admin, admin);
    }

    #[test]
    fn test_register_user() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let user = Address::generate(&env);
        let name = String::from_str(&env, "Dr. Smith");

        client.register_user(&user, &Role::Optometrist, &name);
        let events = env.events().all();

        let user_data = client.get_user(&user);
        assert_eq!(user_data.role, Role::Optometrist);
        assert!(user_data.is_active);

        assert!(!events.is_empty());
        let event = events.get(events.len() - 1).unwrap();
        assert_eq!(
            event.1,
            (symbol_short!("USR_REG"), user.clone()).into_val(&env)
        );
        let payload: events::UserRegisteredEvent = event.2.try_into_val(&env).unwrap();
        assert_eq!(payload.user, user);
        assert_eq!(payload.role, Role::Optometrist);
        assert_eq!(payload.name, name);
    }

    #[test]
    fn test_add_and_get_record() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        let data_hash = String::from_str(&env, "QmHash123");

        let record_id =
            client.add_record(&patient, &provider, &RecordType::Examination, &data_hash);
        let events = env.events().all();

        let record = client.get_record(&record_id);
        assert_eq!(record.patient, patient);
        assert_eq!(record.provider, provider);

        assert!(!events.is_empty());
        let event = events.get(events.len() - 1).unwrap();
        assert_eq!(
            event.1,
            (symbol_short!("REC_ADD"), patient.clone(), provider.clone()).into_val(&env)
        );
        let payload: events::RecordAddedEvent = event.2.try_into_val(&env).unwrap();
        assert_eq!(payload.record_id, record_id);
        assert_eq!(payload.patient, patient);
        assert_eq!(payload.provider, provider);
        assert_eq!(payload.record_type, RecordType::Examination);
    }

    #[test]
    fn test_access_control() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let doctor = Address::generate(&env);

        // Initially no access
        assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);

        // Grant access
        client.grant_access(&patient, &doctor, &AccessLevel::Read, &86400);
        let events = env.events().all();

        // Assert access granted event
        assert!(!events.is_empty());
        let grant_event = events
            .iter()
            .find(|e| {
                let t: &soroban_sdk::Vec<soroban_sdk::Val> = &e.1;
                if !t.is_empty() {
                    let topic0: soroban_sdk::Symbol = t.get(0).unwrap().into_val(&env);
                    return topic0 == symbol_short!("ACC_GRT");
                }
                false
            })
            .expect("ACC_GRT event not found");

        assert_eq!(
            grant_event.1,
            (symbol_short!("ACC_GRT"), patient.clone(), doctor.clone()).into_val(&env)
        );
        let grant_payload: events::AccessGrantedEvent = grant_event.2.try_into_val(&env).unwrap();
        assert_eq!(grant_payload.patient, patient);
        assert_eq!(grant_payload.grantee, doctor);
        assert_eq!(grant_payload.level, AccessLevel::Read);
        assert_eq!(grant_payload.duration_seconds, 86400);

        assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

        // Revoke access
        client.revoke_access(&patient, &doctor);
        let all_events = env.events().all();

        assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
        let revoke_event = all_events.get(all_events.len() - 1).unwrap();
        assert_eq!(
            revoke_event.1,
            (symbol_short!("ACC_REV"), patient.clone(), doctor.clone()).into_val(&env)
        );
        let revoke_payload: events::AccessRevokedEvent = revoke_event.2.try_into_val(&env).unwrap();
        assert_eq!(revoke_payload.patient, patient);
        assert_eq!(revoke_payload.grantee, doctor);
    }
}
