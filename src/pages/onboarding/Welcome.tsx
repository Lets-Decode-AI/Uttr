import Icon, { type IconName } from '../../components/ui/Icon';
import Logo from '../../components/ui/Logo';
import { currentPlatform } from '../../components/ui/Keycap';
import { StepFrame, type StepProps } from './Shell';

const HIGHLIGHTS: { icon: IconName; title: string; text: string }[] = [
  { icon: 'mic', title: 'Talk in any app', text: 'Hold a shortcut in your email, chat or editor and speak. Your words appear at the cursor.' },
  { icon: 'sparkle', title: 'Clean, ready-to-send text', text: 'Optional AI removes filler words and follows the style of each app.' },
  { icon: 'check', title: 'Private by default', text: 'Speech is processed on your computer. No account, no subscription.' },
];

export default function Welcome({ onNext }: Pick<StepProps, 'onNext'>) {
  const where = currentPlatform() === 'mac' ? 'this Mac' : 'this computer';
  return (
    <StepFrame onNext={onNext} nextLabel="Continue" note="Setup takes about two minutes.">
      <div className="onb-glyph"><Logo size={72} /></div>
      <h1>uttr</h1>
      <p className="onb-lead">Dictate into any app. Speech is turned into text on {where} and never leaves it.</p>
      <ul className="onb-highlights" aria-label="What uttr does">
        {HIGHLIGHTS.map((h) => (
          <li key={h.title} className="onb-highlight">
            <span className="onb-highlight-icon" aria-hidden="true"><Icon name={h.icon} size={16} /></span>
            <span><strong>{h.title}</strong><span className="muted">{h.text}</span></span>
          </li>
        ))}
      </ul>
    </StepFrame>
  );
}
