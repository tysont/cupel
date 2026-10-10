package ledger

import (
	"testing"

	"github.com/gkampitakis/go-snaps/snaps"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/mock"
	"github.com/stretchr/testify/require"
)

type MockNotifier struct {
	mock.Mock
}

func (m *MockNotifier) Notify(owner, statement string) error {
	args := m.Called(owner, statement)
	return args.Error(0)
}

func TestTallyReportsOverdraft(t *testing.T) {
	total, overdrawn := tally([]int64{5, -10, 2})
	assert.Equal(t, int64(-3), total)
	assert.True(t, overdrawn)
}

func TestParseCentsAcceptsDigits(t *testing.T) {
	cents, err := parseCents(" 125 ")
	require.NoError(t, err)
	assert.Equal(t, int64(125), cents)
}

func TestParseCentsRejectsBlanks(t *testing.T) {
	_, err := parseCents("  ")
	assert.ErrorIs(t, err, ErrEmpty)
}

func TestDiscountSnapshot(t *testing.T) {
	snaps.MatchSnapshot(t, discounted(200, 50, true))
}

func TestMailNotifiesTheOwner(t *testing.T) {
	notifier := &MockNotifier{}
	notifier.On("Notify", "ada", mock.Anything).Return(nil)
	require.NoError(t, Mail(notifier, NewAccount("ada")))
	notifier.AssertExpectations(t)
}
