import { forwardRef, type ButtonHTMLAttributes } from 'react';
import Icon, { type IconName } from './Icon';
import Tooltip from './Tooltip';

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'aria-label'> {
  icon: IconName;
  label: string;
  tone?: 'default' | 'destructive';
}

/** Borderless icon-only button. The label is required so it always has an accessible name. */
const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton({ icon, label, tone = 'default', className, type = 'button', ...rest }, ref) {
  return (
    <Tooltip content={label}>
      <button ref={ref} type={type} aria-label={label} className={`ui-iconbtn${tone === 'destructive' ? ' ui-iconbtn-destructive' : ''}${className ? ` ${className}` : ''}`} {...rest}>
        <Icon name={icon} size={16} />
      </button>
    </Tooltip>
  );
});

export default IconButton;
