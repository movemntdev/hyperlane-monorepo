module hp_validator::validator_announce {
  use std::vector;
  use std::string::{Self, String};
  use sui::clock::{Self, Clock};
  use sui::coin::{Self, Coin};
  use sui::balance::{Self, Balance};
  use sui::sui::{SUI};
  use sui::object::{Self, ID, UID};
  use sui::transfer;
  use sui::tx_context::{Self, TxContext};
  use sui::pay;
  use sui::event;
  use sui::vec_map::{Self, VecMap};
  use sui::bcs;

  use hp_library::utils::{Self, hash_concat};

  //
  // Constants
  //
  const ERROR_ANNOUNCE_REPLAY: u64 = 0;
  const ERROR_INVALID_SIGNATURE: u64 = 1;
  const ERROR_INVALID_ACCOUNT: u64 = 2;
  const ERROR_INVALID_VALIDATOR_SIGN: u64 = 3;

  //
  // Resources
  //

  // event resources
  struct AnnouncementEvent has store, drop, copy {
    validator: address,
    storage_location: String,
  }

  /// Admin Capability
  struct AdminCap has key, store {
      id: UID,
  }

  struct ValidatorState has key, store {
    id: UID,
    mailbox: address,
    domain: u32,
    storage_locations: VecMap<address, vector<String>>,
    replay_protection: vector<vector<u8>>,
    validators_list: vector<address>,
  }

  fun init(ctx: &mut TxContext) {
    let sender = tx_context::sender(ctx);
    transfer::transfer(AdminCap { id: object::new(ctx) }, sender);
  }

  public entry fun create_state(
    _admin_cap: &AdminCap,
    mailbox: address,
    domain: u32,
    ctx: &mut TxContext
  ) {
    
    transfer::share_object(ValidatorState {
      id: object::new(ctx),
      mailbox,
      domain,
      storage_locations: vec_map::empty<address, vector<String>>(),
      replay_protection: vector::empty<vector<u8>>(),
      validators_list: vector::empty<address>(),
    });

  }

  public entry fun announce(
    validator_state: &mut ValidatorState,
    validator: address,
    signature: vector<u8>,
    storage_location: String,
    ctx: &mut TxContext
  ) acquires ValidatorState {

    // Ensure that the same storage metadata isn't being announced
    // multiple times for the same validator.
    let replay_id = hash_concat(
      bcs::to_bytes(&validator), 
      *string::bytes(&storage_location)
    );
    assert!(!vector::contains(&validator_state.replay_protection, &replay_id), ERROR_ANNOUNCE_REPLAY);
    vector::push_back(&mut validator_state.replay_protection, replay_id);

    // Verify that the signature matches the declared validator
    verify_validator_signed_announcement_internal(
      validator_state, 
      validator,
      signature,
      storage_location
    );
    
    // Store the announcement, Update storage locations
    if (!vector::contains(&validator_state.validators_list, &validator)) {
      vector::push_back(&mut validator_state.validators_list, validator);
      vec_map::insert(&mut validator_state.storage_locations, validator, vector::empty<String>());
    };
    let locations = vec_map::get_mut<address, vector<String>>(
      &mut validator_state.storage_locations,
      &validator
    );
    vector::push_back(locations, storage_location);

    // emit events
    event::emit(AnnouncementEvent {
      validator,
      storage_location
    });
  }

  fun verify_validator_signed_announcement_internal(
    validator_state: &ValidatorState,
    validator: address,
    signature: vector<u8>,
    storage_location: String
  ) {
    let hash_msg = hash_concat(
      utils::announcement_digest(
        validator_state.mailbox,
        validator_state.domain,
      ),
      *string::bytes(&storage_location)
    );

    let announcement_digest = utils::eth_signed_message_hash(&hash_msg);
    let signer_address_bytes = utils::secp256k1_recover_ethereum_address(
      &announcement_digest,
      &signature
    );

    assert!(utils::compare_bytes_and_address(&signer_address_bytes, &validator), ERROR_INVALID_VALIDATOR_SIGN);
  }
  

  #[view]
  /// Returns a list of all announced storage locations
  /// @param `_validators` The list of validators to get registrations for
  /// @return A list of registered storage metadata
  public fun get_announced_storage_locations(validator_state: &ValidatorState, validator_list: vector<address>): vector<vector<String>> acquires ValidatorState {
    let result = vector::empty<vector<String>>();
    let i = 0;
    // loop all validator addresses from parameter
    while (i < vector::length(&validator_list)) {
      let validator = vector::borrow(&validator_list, i);
      // find validator's storage_locations
      if (vec_map::contains(&validator_state.storage_locations, validator)) {  
        let storage_locations = vec_map::get(&validator_state.storage_locations, validator);
        vector::push_back(&mut result, *storage_locations);
      };
      i = i + 1;
    };
    result
  }

  #[view]
  /// Returns a list of validators that have made announcements
  public fun get_announced_validators(validator_state: &ValidatorState): vector<address> acquires ValidatorState {
    validator_state.validators_list
  }

  #[test_only]
  use hp_library::test_utils::{Self, scenario};

  #[test_only]
  use sui::test_scenario::{Self, Scenario, next_tx, ctx};


  #[test]
  fun verify_signature_test() {
    
    let admin = @0xA;
    let scenario_val = scenario();
    let scenario = &mut scenario_val;
    
    let mailbox: address = @0x35231d4c2d8b8adcb5617a638a0c4548684c7c70;
    let domain: u32 = 1;
    let validator: address = @0x4c327ccb881a7542be77500b2833dc84c839e7b7;
    let storage_location: String = string::utf8(b"s3://hyperlane-mainnet2-ethereum-validator-0/us-east-1");
    
    next_tx(scenario, admin);
    {
      let ctx = test_scenario::ctx(scenario);
      // init envs
      init(ctx);
    };

    
    next_tx(scenario, admin);
    {
      let admin_cap = test_scenario::take_from_address<AdminCap>(scenario, admin);
      create_state(&admin_cap, mailbox, domain, test_scenario::ctx(scenario));
      test_scenario::return_to_address(admin, admin_cap);
    };

    let signature = x"20ac937917284eaa3d67287278fc51875874241fffab5eb5fd8ae899a7074c5679be15f0bdb5b4f7594cefc5cba17df59b68ba3c55836053a23307db5a95610d1b";
    
    next_tx(scenario, admin);
    {
      let validator_state = test_scenario::take_shared<ValidatorState>(scenario);
      verify_validator_signed_announcement_internal(
        &mut validator_state,
        validator,
        signature,
        storage_location
      );
      test_scenario::return_shared(validator_state);
    };


    next_tx(scenario, admin);
    {
      let validator_state = test_scenario::take_shared<ValidatorState>(scenario);

      announce(
        &mut validator_state,
        validator,
        signature,
        storage_location,
        test_scenario::ctx(scenario)
      );
      
      assert!(get_announced_validators(&validator_state) == vector[validator], 1);
      assert!(get_announced_storage_locations(&validator_state, vector[validator]) == vector[vector[storage_location]], 2);
      test_scenario::return_shared(validator_state);
    };

    test_scenario::end(scenario_val);
  }
}