use crate::amount::AmountPool;
use crate::envelope::EnvelopeType;
use crate::envelope::builder::EnvelopeBuilder;
use crate::errors::SilverfoxResult;

use super::Account;

pub struct AccountBuilder {
    name: String,
    envelope_builders: Vec<EnvelopeBuilder>,
}

impl AccountBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            envelope_builders: Vec::new(),
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn envelope(mut self, builder: EnvelopeBuilder) -> Self {
        self.envelope_builders.push(builder);
        self
    }

    pub fn build(self) -> SilverfoxResult<Account> {
        let (expense_envelopes, goal_envelopes) = {
            let (mut e, mut g) = (Vec::new(), Vec::new());

            for builder in self.envelope_builders {
                let envelope = builder.build()?;
                match envelope.get_type() {
                    EnvelopeType::Expense => e.push(envelope),
                    EnvelopeType::Goal => g.push(envelope),
                }
            }

            (e, g)
        };
        Ok(Account {
            name: self.name,
            expense_envelopes,
            goal_envelopes,
            real_value: AmountPool::new(),
        })
    }
}
