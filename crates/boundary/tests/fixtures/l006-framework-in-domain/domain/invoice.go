package domain

import (
	"gorm.io/gorm"
	"github.com/gin-gonic/gin"
)

type Invoice struct {
	gorm.Model
	Amount float64
	Status string
}

func (i *Invoice) ToJSON(c *gin.Context) {
	c.JSON(200, i)
}
