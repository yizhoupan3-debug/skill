# Browser and Paste-Ready Examples

## Browser compose

```bash
open "https://exmail.qq.com"
```

Then paste:

```text
To: prof@uni.edu
Subject: Inquiry regarding Research Opportunities - Yizhou Pan

Dear Professor ...
...
Warm regards,
Yizhou Pan
```

## Homework submission

```text
To: ta@sustech.edu.cn
Subject: Fundamentals of Data Science A Assignment 3 Submission - Yizhou Pan 12312411
Attachment: /path/to/hw3.pdf

Dear TA,

Please find attached my submission for Fundamentals of Data Science A Assignment 3.
If you have any questions, please feel free to contact me.

Thank you.

Best regards,
Yizhou Pan
```

## System mail client fallback

Use only when the user accepts manual attachment handling:

```bash
open "mailto:prof@uni.edu?subject=Inquiry%20regarding%20Research%20Opportunities%20-%20Yizhou%20Pan&body=Dear%20Professor%20..."
```
