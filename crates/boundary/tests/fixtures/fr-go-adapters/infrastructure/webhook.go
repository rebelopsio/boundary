package infrastructure

// WebhookHandler is an exported infrastructure adapter — a driving (primary) adapter
// that receives inbound webhook events and forwards them into the application.
type WebhookHandler struct {
	ID     string
	Secret string
}
