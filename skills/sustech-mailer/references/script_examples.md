# SUSTech Mailer Script Reference

Below are execution examples for sending and composing emails using the provided helper scripts.

### Default mode send
```bash
python3 scripts/send_email.py --to prof@uni.edu --subject "..." --body "text"
```

### Homework mode send
```bash
python3 scripts/send_email.py --to ta@sustech.edu.cn --subject "..." --body-file /tmp/body.txt --mode homework --no-attachments --extra-attachment /path/to/hw.pdf
```

### With CC
```bash
python3 scripts/send_email.py --to prof@uni.edu --subject "..." --body "text" --cc "ta@uni.edu"
```

### Body from file
```bash
python3 scripts/send_email.py --to prof@uni.edu --subject "..." --body-file /tmp/body.txt
```

### Dry-run (connect + auth only, no send)
```bash
python3 scripts/send_email.py --to prof@uni.edu --subject "..." --body "text" --dry-run
```

### Skip default attachments
```bash
python3 scripts/send_email.py --to prof@uni.edu --subject "..." --body "text" --no-attachments
```

### Open exmail.qq.com web compose page (default)
```bash
python3 scripts/open_compose.py --to prof@uni.edu --subject "..." --body "text"
```

### Open system mail client via mailto:
```bash
python3 scripts/open_compose.py --to prof@uni.edu --subject "..." --body-file /tmp/body.txt --mailto
```
