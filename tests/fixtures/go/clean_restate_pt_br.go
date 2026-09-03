package main

func RetentarUpload(id int) {
	// não repete porque o serviço upstream já confirma o recebimento
	upload(id)
}
