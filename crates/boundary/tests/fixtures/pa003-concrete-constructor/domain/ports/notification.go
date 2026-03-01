package ports

// NotificationService defines the contract for sending notifications.
type NotificationService interface {
	Send(to string, message string) error
}
