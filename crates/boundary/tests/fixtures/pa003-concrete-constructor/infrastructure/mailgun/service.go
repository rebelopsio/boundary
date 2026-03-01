package mailgun

// MailGunService implements notification sending via Mailgun.
type MailGunService struct {
	apiKey string
}

// NewMailGunService returns a concrete type instead of ports.NotificationService.
func NewMailGunService(apiKey string) *MailGunService {
	return &MailGunService{apiKey: apiKey}
}

func (s *MailGunService) Send(to string, message string) error {
	return nil
}
