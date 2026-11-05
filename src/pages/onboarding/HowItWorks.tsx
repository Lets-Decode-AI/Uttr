import { keycapsFor } from '../../components/ui/Keycap';
import { useBinding } from '../../lib/useBinding';
import { StepFrame, type StepProps } from './Shell';

/** An animated picture of a dictation: click a text box, hold the keys, speak, let go. */
function Demo() {
  const { binding } = useBinding();
  return (
    <div className="demo" role="img" aria-label="Hold the shortcut, speak, and let go: your words appear where you are typing.">
      <div className="demo-window">
        <div className="demo-bar"><span /><span /><span /></div>
        <div className="demo-field">
          <span className="demo-text">Can we move the review to Thursday?</span>
          <span className="demo-caret" />
        </div>
      </div>
      <div className="demo-keys" aria-hidden="true">{keycapsFor(binding)}</div>
      <div className="demo-pill" aria-hidden="true">{[0, 1, 2, 3, 4, 5, 6].map((i) => <span key={i} style={{ animationDelay: `${i * 90}ms` }} />)}</div>
    </div>
  );
}

export default function HowItWorks(props: StepProps) {
  const { binding } = useBinding();
  return (
    <StepFrame {...props}>
      <h1>How it works</h1>
      <Demo />
      <ol className="onb-moves">
        <li><strong>Click into any text box</strong>, then hold {keycapsFor(binding)} while you talk.</li>
        <li><strong>Let go</strong> and your words appear. The Flow Bar at the bottom of the screen shows when uttr is listening.</li>
        <li><strong>Double-press</strong> the shortcut to keep listening hands-free. <strong>Esc cancels</strong>.</li>
      </ol>
    </StepFrame>
  );
}
