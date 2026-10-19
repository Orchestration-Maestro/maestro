//! Device, window and active-display selectors.

use crate::stream::QueryAction;

use super::super::{Action, Handler};
use crate::{csi::SizeReportStyle as S, device_attributes::Req, parser::Csi};

/// Emit typed requests without executing terminal effects.
pub(super) fn dispatch(input: &Csi<'_>, handler: &mut impl Handler) {
    let action = match (input.r#final, &*input.intermediates, &*input.params) {
        (b'c', [], _) => Action::Query(QueryAction::DeviceAttributes(Req::Primary)),
        (b'c', [b'>'], _) => Action::Query(QueryAction::DeviceAttributes(Req::Secondary)),
        (b'c', [b'='], _) => Action::Query(QueryAction::DeviceAttributes(Req::Tertiary)),
        (b't', [], [14]) => Action::Query(QueryAction::SizeReport(S::Csi14T)),
        (b't', [], [16]) => Action::Query(QueryAction::SizeReport(S::Csi16T)),
        (b't', [], [18]) => Action::Query(QueryAction::SizeReport(S::Csi18T)),
        (b't', [], [21]) => Action::Query(QueryAction::SizeReport(S::Csi21T)),
        (b't', [], [22, 0 | 2]) => Action::Query(QueryAction::TitlePush(0)),
        (b't', [], [23, 0 | 2]) => Action::Query(QueryAction::TitlePop(0)),
        (b't', [], [22, 0 | 2, index]) => Action::Query(QueryAction::TitlePush(*index)),
        (b't', [], [23, 0 | 2, index]) => Action::Query(QueryAction::TitlePop(*index)),
        (b'}', [b'$'], [0]) => Action::ActiveStatusDisplay(crate::ansi::StatusDisplay::Main),
        (b'}', [b'$'], [1]) => Action::ActiveStatusDisplay(crate::ansi::StatusDisplay::StatusLine),
        _ => return,
    };
    handler.vt(action);
}

/// Retain zero operands and defer the parameterless horizontal ambiguity.
pub(super) fn margins(input: &Csi<'_>, handler: &mut impl Handler) {
    let (top_left, bottom_right) = match &*input.params {
        [] if input.r#final == b's' => {
            handler.vt(Action::LeftAndRightMarginAmbiguous);
            return;
        }
        [] => (0, 0),
        [first] => (*first, 0),
        [first, second] => (*first, *second),
        _ => return,
    };
    handler.vt(if input.r#final == b'r' {
        Action::TopAndBottomMargin {
            top_left,
            bottom_right,
        }
    } else {
        Action::LeftAndRightMargin {
            top_left,
            bottom_right,
        }
    });
}
