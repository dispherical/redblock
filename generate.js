const fs = require('fs');
const { parse } = require('csv-parse');
const { IpAddress, IpRange } = require('cidr-calc');
const cliProgress = require('cli-progress');
const inputFile = 'dbip-city-lite-2025-10.csv';
const outputFile = 'output.txt';
const input = fs.createReadStream(inputFile);
const output = fs.createWriteStream(outputFile);

const regions = {}; 

const blocklistStates = [
  "Alabama",
  "Arkansas",
  "Florida",
  "Georgia",
  "Idaho",
  "Indiana",
  "Kansas",
  "Kentucky",
  "Louisiana",
  "Mississippi",
  "Missouri",
  "Montana",
  "Nebraska",
  "North Carolina",
  "North Dakota",
  "Ohio",
  "Oklahoma",
  "South Carolina",
  "South Dakota",
  "Tennessee",
  "Texas",
  "Utah",
  "Virginia",
  "Wyoming"
];

const blocklistCountries = [
  "GB",
  "FR",
  "DE",
  "IT",
  "DK",
  "AU"
];


const parser = parse({
  from_line: 1,
  columns: false,
  skip_empty_lines: true,
});

let totalLines = 0;
let counted = false;
let bar;
function initProgressBar(cb) {
  if (counted) return cb();
  counted = true;
  let lineCount = 0;
  fs.createReadStream(inputFile)
    .on('data', chunk => {
      for (let i = 0; i < chunk.length; ++i) {
        if (chunk[i] === 10) lineCount++;
      }
    })
    .on('end', () => {
      totalLines = lineCount;
      bar = new cliProgress.SingleBar({
        format: 'Processing |{bar}| {percentage}% | {value}/{total} lines',
        hideCursor: true
      }, cliProgress.Presets.shades_classic);
      bar.start(totalLines, 0);
      cb();
    });
}


let processed = 0;
parser.on('readable', () => {
  let record;
  while ((record = parser.read())) {
    processed++;
    if (bar) bar.update(processed);
    const [ipStart, ipEnd, continent, country, stateprov] = record;
    
    let shouldBlock = false;
    let label = '';
    
    if (country === 'US' && blocklistStates.includes(stateprov)) {
      shouldBlock = true;
      label = `US ${stateprov}`;
    }
    else if (blocklistCountries.includes(country)) {
      shouldBlock = true;
      label = country;
    }
    
    if (shouldBlock) {
      if (!regions[label]) regions[label] = new Set();
      try {
        const range = new IpRange(
          IpAddress.of(ipStart),
          IpAddress.of(ipEnd)
        );
        const cidrs = range.toCidrs();
        cidrs.forEach(c => regions[label].add(c.toString()));
      } catch (err) {
        console.warn(`Skipping invalid range ${ipStart}-${ipEnd}:`, err.message);
      }
    }
  }
});



parser.on('end', () => {
  if (bar) bar.update(totalLines);
  if (bar) bar.stop();

  let totalCidrs = 0;
  let totalIps = 0;
  Object.values(regions).forEach(cidrSet => {
    cidrSet.forEach(cidrStr => {
      totalCidrs++;
      try {
        const [ip, prefix] = cidrStr.split('/');
        const prefixNum = parseInt(prefix, 10);
        let ipCount = 0;
        
        const isIPv6 = ip.includes(':');
        
        if (isIPv6) {
          if (!isNaN(prefixNum) && prefixNum >= 0 && prefixNum <= 128) {
            const hostBits = 128 - prefixNum;
            if (hostBits <= 53) {
              ipCount = Math.pow(2, hostBits);
            } else {
              ipCount = Number(2n ** BigInt(hostBits));
              if (ipCount === Infinity || hostBits > 60) {
                ipCount = Number.MAX_SAFE_INTEGER;
              }
            }
          } else {
            ipCount = 1;
          }
        } else {
          if (!isNaN(prefixNum) && prefixNum >= 0 && prefixNum <= 32) {
            ipCount = Math.pow(2, 32 - prefixNum);
          } else {
            ipCount = 1;
          }
        }
        
        totalIps += ipCount;
      } catch (e) {
        totalIps += 1;
      }
    });
  });

  const headerPlaceholder = `# This is a database of all IPs with legislation impacting free speech\n# Updated ${new Date()}\n# Total CIDR entries: ${totalCidrs}\n# Total IPs blocked: ${totalIps}\n# File size: calculating...\n# Includes transformed data from DB-IP (db-ip.com) licensed under CC BY 4.0\n\n`;
  output.write(headerPlaceholder);
  let headerLength = Buffer.byteLength(headerPlaceholder);

  Object.keys(regions).sort().forEach(region => {
    output.write(`# ${region}\n`);
    regions[region].forEach(cidr => {
      output.write(cidr + '\n');
    });
    output.write('\n');
  });
  output.end(async () => {
    const stats = fs.statSync(outputFile);
    let fileSize = stats.size;
    const mb = (fileSize / (1024 * 1024)).toFixed(2);
    const header = `# A list of IP ranges (in CIDR format, newline-delimited) registered to U.S. states and countries that have enacted laws requiring age verification for online content deemed "harmful to minors."\n# These laws are often written in vague or overly broad terms, and in practice, they have been used or proposed to restrict access to LGBTQ+ content, sexual health information, and other constitutionally protected material.\n# Updated ${new Date()}\n# Total CIDR entries: ${totalCidrs}\n# Total IPs blocked: ${totalIps}\n# File size: ${mb} MB, ${fileSize} bytes\n# Learn more at https://dispherical.com/tools/redblock/\n# Note: Georgia, South Dakota, and Wyoming have been included as the bills went into force 1 July 2025\n# Countries included: United Kingdom (GB), France (FR), Germany (DE), Italy (IT), Denmark (DK), Australia (AU)\n# Includes transformed data from DB-IP (db-ip.com) licensed under CC BY 4.0\n\n`;
    const rest = fs.readFileSync(outputFile).slice(headerLength);
    fs.writeFileSync(outputFile, header);
    fs.appendFileSync(outputFile, rest);
    fs.writeFileSync('header.txt', header);
    console.log('Done.');
    process.exit(0);
  });
});

initProgressBar(() => {
  input.pipe(parser);
});
