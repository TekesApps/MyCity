//! Public natural-needs smoke; original private component cases run in library tests.
use mycity_core::{arena,model_rows,service_needs::*};
#[test]fn public_natural_profile_remains_available_with_exact_current_layout(){
 assert_eq!(std::mem::size_of::<Current>(),16);let mut households=model_rows::Rows::new().unwrap();households.try_push(Household{household_id:1,source_id:101,generation:1,people:2}).unwrap();
 let opening=Opening{version:1,profile:Profile::LocalDevelopmentNaturalNeedsV1,scope:arena::Scope::new(1,2).unwrap(),city_id:7,seed:0,households};let r=Runtime::new(opening,0).unwrap();assert_eq!(r.next_due().unwrap(),None);
}
