
#[cfg(feature = "lang-en_US")]
include!("en_US.rs");
#[cfg(feature = "lang-pt_BR")]
include!("pt_BR.rs");

#[cfg(not(any(
	feature = "lang-en_US",
	feature = "lang-pt_BR",
)))]
std::compile_error!("No string set has been selected.");
