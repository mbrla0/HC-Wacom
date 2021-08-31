

/// Strings used in the tablet management window.
pub mod manager {
	pub fn title() -> &'static str { "Assinatura para Termo de Consentimento" }
	pub fn help_btn() -> &'static str { "Ajuda" }
	pub fn help() -> &'static str {
		"\
			Instruções para coleta de assinatura.\
			\n\
			\n1) No TLCE digital, clique em \"Adicionar Assinatura\";\
			\n2) Solicite ao paciente que assine no dispositivo;\
			\n3) Clique em Pintar e vá para a janela do TCLE;\
			\n4) Selecione a área de assinatura;\
			\n5) Aperte a Tecla 'e' para adicionar a assitura;\
			\n6) Clique em \"Salvar Assintura\" no TCLE digital.\
		"
	}
	pub fn display_clear_btn() -> &'static str { "Limpar" }
	pub fn display_paint_btn() -> &'static str { "Pintar" }
	pub fn display_label() -> &'static str { "Oncologia Clínica - HC FMRP - USP" }
}

/// Strings used in the device selection window.
pub mod selector {
	pub fn title() -> &'static str { "Assinatura para Termo de Consentimento" }
	pub fn description() -> &'static str { "Selecione o dispositivo ao qual deseja se conectar." }
	pub fn cancel() -> &'static str { "Cancelar" }
	pub fn accept() -> &'static str { "Conectar" }
}

/// Strings used in the area selection window.
pub mod area {
	pub fn tip() -> &'static str {
		"Selecione uma regiao clicando e arrastando em qualquer parte da tela. \
		Pressione 'e' para confirmar a regiao selecionada e 'q' para cancelar."
	}
}

/// Strings used in error messages.
pub mod errors {
	pub fn title() -> &'static str { "Erro" }
	pub fn signature_paint_pick_area_failed(
		what: crate::window::PickPhysicalAreaError) -> String {
		format!("Não foi possível mostrar a seleção de região de pintura: {}",
			what)
	}
	pub fn no_tablets_available() -> &'static str {
		"Não há dispositivos de entrada de assinatura disponíveis neste sistema"
	}
	pub fn device_prompt_creation_failed(
		what: nwg::NwgError) -> String {
		format!("Não foi possível criar a janela de seleção de dispositivo de \
			entrada: {}", what)
	}
	pub fn tablet_not_found(
		information: stu::Information) -> String {
		format!(
			"Não foi possível encontrar o dispositivo \"{} - {:04x}:{:04x}\". \
			Certifique-se que esse não foi desconectado.",
			information.device(), information.vendor(), information.product())
	}
	pub fn tablet_connection_failed(
		information: stu::Information,
		what: stu::Error) -> String {
		format!(
			"\
				Não foi possível conectar-se ao dispositivo \
				\"{} - {:04x}:{:04x}\": {}.\n\n\
				\
				Error: {:?}\
			",
			information.device(),
			information.vendor(),
			information.product(),
			what, what)
	}
	pub fn management_failed(
		what: crate::window::ManagementError) -> String {
		format!(
			"Occoreu um erro durante a supervisão do dispositivo: {}",
			what)
	}
}
