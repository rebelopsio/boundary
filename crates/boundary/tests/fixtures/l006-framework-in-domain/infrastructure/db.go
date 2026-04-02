package infrastructure

import "gorm.io/gorm"

type GormInvoiceRepository struct {
	db *gorm.DB
}

func (r *GormInvoiceRepository) Save(amount float64) error {
	return nil
}
