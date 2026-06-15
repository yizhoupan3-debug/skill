use crate::cli::*;

impl ModeArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ModeArg::Quick => "quick",
            ModeArg::Full => "full",
        }
    }
}

impl PriorityArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            PriorityArg::High => "high",
            PriorityArg::Medium => "medium",
            PriorityArg::Low => "low",
        }
    }
}

impl OutcomeArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            OutcomeArg::Confirmatory => "confirmatory",
            OutcomeArg::Exploratory => "exploratory",
            OutcomeArg::Failed => "failed",
            OutcomeArg::Ambiguous => "ambiguous",
        }
    }
}

impl DirectionArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            DirectionArg::Deepen => "DEEPEN",
            DirectionArg::Broaden => "BROADEN",
            DirectionArg::Pivot => "PIVOT",
            DirectionArg::Conclude => "CONCLUDE",
        }
    }
}

impl GateStatusArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            GateStatusArg::Pending => "pending",
            GateStatusArg::Passed => "passed",
            GateStatusArg::Pivot => "pivot",
        }
    }
}

impl ExternalSourceArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ExternalSourceArg::All => "all",
            ExternalSourceArg::SemanticScholar => "semantic-scholar",
            ExternalSourceArg::Arxiv => "arxiv",
        }
    }
}

impl OverlapArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            OverlapArg::Low => "low",
            OverlapArg::Medium => "medium",
            OverlapArg::High => "high",
        }
    }
}

impl ConfidenceArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ConfidenceArg::Low => "low",
            ConfidenceArg::Medium => "medium",
            ConfidenceArg::High => "high",
        }
    }
}

impl VerdictArg {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            VerdictArg::Novel => "novel",
            VerdictArg::Defensible => "defensible",
            VerdictArg::Risky => "risky",
            VerdictArg::NotNovel => "not-novel",
        }
    }
}
